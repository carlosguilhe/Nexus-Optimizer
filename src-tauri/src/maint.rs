use serde::Serialize;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ------------------------------------------------------ SFC e DISM (com progresso)

static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone)]
struct MaintEvent {
    task: &'static str,
    progress: Option<f64>,
    line: String,
}

/// Número imediatamente antes de um "%" ("Verificação 45% concluída", "[==== 45.0% ]").
pub fn parse_percent(text: &str) -> Option<f64> {
    let end = text.rfind('%')?;
    let digits: String = text[..end]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.replace(',', ".").parse().ok()
}

/// O sfc escreve em UTF-16LE; o DISM em página de código do console.
fn decode(bytes: &[u8], utf16: bool) -> String {
    if utf16 {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units).replace('\0', "")
    } else {
        String::from_utf8_lossy(bytes).to_string()
    }
}

/// Roda uma ferramenta do Windows lendo a saída aos poucos e enviando o progresso (evento "maint").
pub fn run_tool(app: &AppHandle, task: &'static str) -> Result<String, String> {
    if !crate::tweaks::is_admin() {
        return Err("Esta verificação exige administrador.".into());
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Já há uma verificação em andamento.".into());
    }
    let (exe, args, utf16): (&str, &[&str], bool) = match task {
        "sfc" => ("sfc", &["/scannow"], true),
        "dism" => ("DISM", &["/Online", "/Cleanup-Image", "/RestoreHealth"], false),
        _ => {
            RUNNING.store(false, Ordering::SeqCst);
            return Err("tarefa desconhecida".into());
        }
    };
    let result = (|| {
        let mut child = Command::new(exe)
            .args(args)
            .stdout(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut out = child.stdout.take().ok_or("sem saída")?;
        let (mut buf, mut all) = ([0u8; 2048], Vec::<u8>::new());
        loop {
            let n = out.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            all.extend_from_slice(&buf[..n]);
            let tail = &all[all.len().saturating_sub(400)..];
            let text = decode(&tail[tail.len() % 2..], utf16);
            if let Some(p) = parse_percent(&text) {
                let line = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
                let _ = app.emit("maint", MaintEvent { task, progress: Some(p), line });
            }
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        let text = decode(&all, utf16);
        let summary: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.contains('%')).collect();
        let tail = summary[summary.len().saturating_sub(3)..].join(" ");
        if status.success() {
            Ok(tail)
        } else {
            Err(if tail.is_empty() { format!("terminou com código {:?}", status.code()) } else { tail })
        }
    })();
    RUNNING.store(false, Ordering::SeqCst);
    result
}

// -------------------------------------------------------- analisador de disco

static CANCEL: AtomicBool = AtomicBool::new(false);

pub fn cancel_scan() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[derive(Serialize, Debug)]
pub struct DirSize {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub files: u64,
    pub is_dir: bool,
}

#[derive(Serialize, Clone)]
struct ScanEvent {
    current: String,
    done: usize,
    total: usize,
}

/// Soma recursivamente (sem seguir links). Para antes se o usuário cancelar.
fn dir_size(root: &Path) -> (u64, u64) {
    let (mut bytes, mut files) = (0u64, 0u64);
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        if CANCEL.load(Ordering::Relaxed) {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(e.path());
            } else if let Ok(md) = e.metadata() {
                bytes += md.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

/// Tamanho de cada item dentro de `root`, do maior para o menor.
pub fn scan(app: &AppHandle, root: &str) -> Result<Vec<DirSize>, String> {
    let root = PathBuf::from(root);
    if !root.is_dir() {
        return Err("pasta inexistente".into());
    }
    CANCEL.store(false, Ordering::SeqCst);
    let children: Vec<_> = std::fs::read_dir(&root).map_err(|e| e.to_string())?.flatten().collect();
    let total = children.len();
    let mut out = Vec::new();
    let (mut loose_bytes, mut loose_files) = (0u64, 0u64);
    for (i, c) in children.iter().enumerate() {
        if CANCEL.load(Ordering::Relaxed) {
            return Err("análise cancelada".into());
        }
        let name = c.file_name().to_string_lossy().to_string();
        let _ = app.emit("diskscan", ScanEvent { current: name.clone(), done: i, total });
        let Ok(ft) = c.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            let (bytes, files) = dir_size(&c.path());
            out.push(DirSize { name, path: c.path().to_string_lossy().to_string(), bytes, files, is_dir: true });
        } else if let Ok(md) = c.metadata() {
            loose_bytes += md.len();
            loose_files += 1;
        }
    }
    if loose_files > 0 {
        out.push(DirSize { name: String::new(), path: root.to_string_lossy().to_string(), bytes: loose_bytes, files: loose_files, is_dir: false });
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out.truncate(60);
    Ok(out)
}

/// Locais de partida para a análise: perfil do usuário e cada unidade fixa.
pub fn disk_roots() -> Vec<(String, String)> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("USERPROFILE") {
        v.push(("Perfil do usuário".to_string(), p));
    }
    for letter in 'C'..='Z' {
        let p = format!("{letter}:\\");
        if Path::new(&p).is_dir() {
            v.push((format!("Unidade {letter}:"), p));
        }
    }
    v
}

// ------------------------------------------------------------ desinstalador

#[derive(Serialize, Debug)]
pub struct Program {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub size_kb: u64,
    pub admin: bool,
}

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

fn bases() -> [(&'static str, bool, &'static str); 3] {
    [("CU", false, UNINSTALL), ("LM", true, UNINSTALL), ("LM32", true, UNINSTALL32)]
}

fn root(hklm: bool) -> RegKey {
    RegKey::predef(if hklm { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

pub fn programs() -> Vec<Program> {
    let mut out = Vec::new();
    for (tag, hklm, path) in bases() {
        let Ok(k) = root(hklm).open_subkey(path) else { continue };
        for sub in k.enum_keys().flatten() {
            let Ok(p) = k.open_subkey(&sub) else { continue };
            let name: String = p.get_value("DisplayName").unwrap_or_default();
            let system: u32 = p.get_value("SystemComponent").unwrap_or(0);
            let has_cmd = p.get_value::<String, _>("UninstallString").is_ok();
            let child = p.get_value::<String, _>("ParentKeyName").is_ok();
            if name.is_empty() || system == 1 || !has_cmd || child {
                continue;
            }
            out.push(Program {
                id: format!("{tag}|{sub}"),
                name,
                version: p.get_value("DisplayVersion").unwrap_or_default(),
                publisher: p.get_value("Publisher").unwrap_or_default(),
                size_kb: p.get_value::<u32, _>("EstimatedSize").map(u64::from).unwrap_or(0),
                admin: hklm,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.name == b.name && a.version == b.version);
    out
}

/// Abre o desinstalador do próprio programa. O comando vem do registro (nunca da interface):
/// a interface só informa qual programa da lista foi escolhido.
pub fn uninstall(id: &str) -> Result<String, String> {
    let (tag, sub) = id.split_once('|').ok_or("programa inválido")?;
    let (_, hklm, base) = bases().into_iter().find(|b| b.0 == tag).ok_or("programa inválido")?;
    if sub.contains('\\') || sub.is_empty() {
        return Err("programa inválido".into());
    }
    let key = root(hklm).open_subkey(format!(r"{base}\{sub}")).map_err(|_| "programa não encontrado")?;
    let name: String = key.get_value("DisplayName").unwrap_or_default();
    let cmd: String = key.get_value("UninstallString").map_err(|_| "este programa não informa como desinstalar")?;
    Command::new("cmd")
        .raw_arg(format!("/C \"{cmd}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_parsing() {
        assert_eq!(parse_percent("Verificação 45% concluída."), Some(45.0));
        assert_eq!(parse_percent("[==========   25.0%   ]"), Some(25.0));
        assert_eq!(parse_percent("Verification 100% complete"), Some(100.0));
        assert_eq!(parse_percent("sem número"), None);
    }

    #[test]
    fn utf16_decoding() {
        let bytes: Vec<u8> = "ok 10%".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(decode(&bytes, true), "ok 10%");
    }

    #[test]
    fn scan_sizes_and_sorts() {
        let root = std::env::temp_dir().join("nexus_scan_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("grande")).unwrap();
        std::fs::create_dir_all(root.join("pequena")).unwrap();
        std::fs::write(root.join("grande").join("a.bin"), vec![0u8; 5000]).unwrap();
        std::fs::write(root.join("pequena").join("b.bin"), vec![0u8; 10]).unwrap();
        let (bytes, files) = dir_size(&root.join("grande"));
        assert_eq!((bytes, files), (5000, 1));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn programs_list_and_uninstall_validation() {
        let list = programs();
        assert!(list.iter().all(|p| !p.name.is_empty()));
        assert!(uninstall("XX|abc").is_err());
        assert!(uninstall("CU|a\\b").is_err());
        assert!(uninstall("CU|chave-que-nao-existe-nexus").is_err());
        assert!(!disk_roots().is_empty());
    }
}
