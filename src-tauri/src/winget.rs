use serde::Serialize;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Catálogo fechado: só estes ids chegam ao winget (a interface nunca envia texto livre).
/// (id do winget, nome, categoria)
const CATALOG: &[(&str, &str, &str)] = &[
    ("Valve.Steam", "Steam", "Lojas de jogos"),
    ("EpicGames.EpicGamesLauncher", "Epic Games Launcher", "Lojas de jogos"),
    ("GOG.Galaxy", "GOG Galaxy", "Lojas de jogos"),
    ("ElectronicArts.EADesktop", "EA app", "Lojas de jogos"),
    ("Ubisoft.Connect", "Ubisoft Connect", "Lojas de jogos"),
    ("Discord.Discord", "Discord", "Comunicação e mídia"),
    ("OBSProject.OBSStudio", "OBS Studio", "Comunicação e mídia"),
    ("VideoLAN.VLC", "VLC", "Comunicação e mídia"),
    ("Spotify.Spotify", "Spotify", "Comunicação e mídia"),
    ("CPUID.CPU-Z", "CPU-Z", "Hardware e monitoramento"),
    ("CPUID.HWMonitor", "HWMonitor", "Hardware e monitoramento"),
    ("TechPowerUp.GPU-Z", "GPU-Z", "Hardware e monitoramento"),
    ("LibreHardwareMonitor.LibreHardwareMonitor", "Libre Hardware Monitor", "Hardware e monitoramento"),
    ("Rem0o.FanControl", "Fan Control", "Hardware e monitoramento"),
    ("Guru3D.Afterburner", "MSI Afterburner", "Hardware e monitoramento"),
    ("Wagnardsoft.DisplayDriverUninstaller", "Display Driver Uninstaller (DDU)", "Hardware e monitoramento"),
    ("7zip.7zip", "7-Zip", "Utilitários"),
    ("Notepad++.Notepad++", "Notepad++", "Utilitários"),
    ("Microsoft.PowerToys", "PowerToys", "Utilitários"),
    ("voidtools.Everything", "Everything (busca de arquivos)", "Utilitários"),
    ("Microsoft.WindowsTerminal", "Windows Terminal", "Utilitários"),
    ("Microsoft.VCRedist.2015+.x64", "Visual C++ Redistributable 2015-2022 (x64)", "Bibliotecas para jogos"),
    ("Microsoft.DotNet.DesktopRuntime.8", ".NET Desktop Runtime 8", "Bibliotecas para jogos"),
    ("Microsoft.DirectX", "DirectX (runtime)", "Bibliotecas para jogos"),
    ("Mozilla.Firefox", "Firefox", "Navegadores"),
    ("Google.Chrome", "Google Chrome", "Navegadores"),
    ("Brave.Brave", "Brave", "Navegadores"),
];

static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
pub struct Package {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub installed: bool,
}

#[derive(Serialize, Clone)]
struct WingetEvent {
    id: String,
    line: String,
    progress: Option<f64>,
}

fn winget() -> Command {
    let mut c = Command::new("winget");
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

pub fn available() -> bool {
    winget().arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Ids do catálogo encontrados numa exportação do winget (`winget export`).
fn ids_in_export(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return vec![];
    };
    v["Sources"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["Packages"].as_array().into_iter().flatten())
        .filter_map(|p| p["PackageIdentifier"].as_str().map(str::to_string))
        .collect()
}

pub fn list() -> Result<Vec<Package>, String> {
    if !available() {
        return Err("O winget não foi encontrado. Instale o \"Instalador de Aplicativo\" pela Microsoft Store.".into());
    }
    let file = crate::tweaks::data_dir()?.join("winget_export.json");
    let _ = std::fs::remove_file(&file);
    let status = winget()
        .args(["export", "-o"])
        .arg(&file)
        .args(["--accept-source-agreements", "--disable-interactivity"])
        .output()
        .map_err(|e| e.to_string())?;
    // Sem pacotes reconhecidos o winget pode não gerar o arquivo: tratamos como "nenhum instalado".
    let json = std::fs::read_to_string(&file).unwrap_or_default();
    let _ = std::fs::remove_file(&file);
    if json.is_empty() && !status.status.success() && status.stdout.is_empty() && status.stderr.is_empty() {
        return Err("O winget não respondeu.".into());
    }
    let have = ids_in_export(&json);
    Ok(CATALOG
        .iter()
        .map(|(id, name, category)| Package {
            id,
            name,
            category,
            installed: have.iter().any(|h| h.eq_ignore_ascii_case(id)),
        })
        .collect())
}

/// Últimas linhas legíveis de uma saída do winget (barras de progresso e spinners ficam de fora).
fn clean_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    if line.is_empty() || line.chars().all(|c| matches!(c, '-' | '\\' | '|' | '/' | ' ')) {
        return None;
    }
    // Linhas de barra de progresso ("███▒▒  45%") viram só o percentual no campo de progresso.
    if line.chars().any(|c| matches!(c, '█' | '▒')) {
        return None;
    }
    Some(line.to_string())
}

fn percent_of(raw: &str) -> Option<f64> {
    crate::maint::parse_percent(raw).filter(|p| (0.0..=100.0).contains(p))
}

fn run(app: &AppHandle, event_id: &str, args: &[&str]) -> Result<String, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Já há uma instalação em andamento.".into());
    }
    let result = (|| {
        let mut child = winget()
            .args(args)
            .args([
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut out = child.stdout.take().ok_or("sem saída")?;
        let (mut buf, mut pending, mut last) = ([0u8; 1024], String::new(), String::new());
        loop {
            let n = out.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            pending.push_str(&String::from_utf8_lossy(&buf[..n]));
            // O winget redesenha a barra com \r; cada trecho vira uma atualização.
            while let Some(i) = pending.find(['\r', '\n']) {
                let piece: String = pending.drain(..=i).collect();
                let progress = percent_of(&piece);
                if let Some(line) = clean_line(&piece) {
                    last = line.clone();
                    let _ = app.emit("winget", WingetEvent { id: event_id.into(), line, progress });
                } else if progress.is_some() {
                    let _ = app.emit("winget", WingetEvent { id: event_id.into(), line: last.clone(), progress });
                }
            }
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(last)
        } else {
            Err(if last.is_empty() { format!("o winget terminou com código {:?}", status.code()) } else { last })
        }
    })();
    RUNNING.store(false, Ordering::SeqCst);
    result
}

fn known(id: &str) -> Result<&'static str, String> {
    CATALOG.iter().find(|(i, _, _)| *i == id).map(|(i, _, _)| *i).ok_or_else(|| "programa fora do catálogo".to_string())
}

pub fn name_of(id: &str) -> String {
    CATALOG.iter().find(|(i, _, _)| *i == id).map(|(_, n, _)| n.to_string()).unwrap_or_else(|| id.to_string())
}

pub fn install(app: &AppHandle, id: &str) -> Result<String, String> {
    let id = known(id)?;
    run(app, id, &["install", "--id", id, "--exact"])
}

pub fn upgrade(app: &AppHandle, id: &str) -> Result<String, String> {
    let id = known(id)?;
    run(app, id, &["upgrade", "--id", id, "--exact"])
}

/// Atualiza tudo o que o winget conhece (não só o catálogo).
pub fn upgrade_all(app: &AppHandle) -> Result<String, String> {
    run(app, "*", &["upgrade", "--all", "--include-unknown"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_safe() {
        let mut ids: Vec<_> = CATALOG.iter().map(|(i, _, _)| *i).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
        for id in ids {
            assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')), "{id}");
        }
    }

    #[test]
    fn only_catalog_ids_are_accepted() {
        assert!(known("Valve.Steam").is_ok());
        assert!(known("Valve.Steam; calc").is_err());
        assert!(known("").is_err());
    }

    #[test]
    fn parses_export() {
        let json = r#"{"Sources":[{"Packages":[{"PackageIdentifier":"Valve.Steam"},{"PackageIdentifier":"X.Y"}]}]}"#;
        assert_eq!(ids_in_export(json), vec!["Valve.Steam", "X.Y"]);
        assert!(ids_in_export("não é json").is_empty());
    }

    #[test]
    fn cleans_progress_lines() {
        assert_eq!(clean_line("  - \r"), None);
        assert_eq!(clean_line("██████▒▒▒  45%"), None);
        assert_eq!(clean_line(" Baixando https://x \n").as_deref(), Some("Baixando https://x"));
        assert_eq!(percent_of("██████▒▒▒  45%"), Some(45.0));
    }

    #[test]
    #[ignore = "usa o winget real"]
    fn lists_real_catalog() {
        let l = list().unwrap();
        assert_eq!(l.len(), CATALOG.len());
    }
}
