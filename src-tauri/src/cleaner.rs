use crate::tools::sweep;
use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize)]
pub struct Category {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub admin: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Report {
    pub id: String,
    pub files: u64,
    pub bytes: u64,
}

struct Def {
    cat: Category,
    /// Só arquivos mais antigos que isso (0 = todos). Protege temporários de programas abertos.
    min_age_hours: u64,
    paths: fn() -> Vec<PathBuf>,
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var(var).ok().map(PathBuf::from)
}

fn local(sub: &str) -> Vec<PathBuf> {
    env_path("LOCALAPPDATA").map(|p| vec![p.join(sub)]).unwrap_or_default()
}

fn temp_user() -> Vec<PathBuf> {
    vec![std::env::temp_dir()]
}

fn temp_windows() -> Vec<PathBuf> {
    env_path("SystemRoot").map(|p| vec![p.join("Temp")]).unwrap_or_default()
}

fn crash() -> Vec<PathBuf> {
    let mut v = local("CrashDumps");
    v.extend(local(r"Microsoft\Windows\WER"));
    if let Some(pd) = env_path("ProgramData") {
        v.push(pd.join(r"Microsoft\Windows\WER\ReportArchive"));
        v.push(pd.join(r"Microsoft\Windows\WER\ReportQueue"));
    }
    v
}

fn thumbs() -> Vec<PathBuf> {
    local(r"Microsoft\Windows\Explorer")
}

fn windows_update() -> Vec<PathBuf> {
    env_path("SystemRoot").map(|p| vec![p.join(r"SoftwareDistribution\Download")]).unwrap_or_default()
}

fn browsers() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for base in [r"Google\Chrome\User Data", r"Microsoft\Edge\User Data", r"BraveSoftware\Brave-Browser\User Data"] {
        for sub in ["Cache", "Code Cache", "GPUCache"] {
            v.extend(local(&format!(r"{base}\Default\{sub}")));
        }
    }
    // Firefox: uma pasta cache2 por perfil.
    if let Some(profiles) = local(r"Mozilla\Firefox\Profiles").pop() {
        if let Ok(rd) = std::fs::read_dir(profiles) {
            v.extend(rd.flatten().map(|e| e.path().join("cache2")));
        }
    }
    v
}

fn defs() -> Vec<Def> {
    let d = |id, name, description, admin, min_age_hours, paths| Def {
        cat: Category { id, name, description, admin },
        min_age_hours,
        paths,
    };
    vec![
        d("temp-user", "Temporários do usuário", "Arquivos temporários com mais de 24 horas.", false, 24, temp_user),
        d("temp-windows", "Temporários do Windows", "Pasta Temp do sistema, com mais de 24 horas.", true, 24, temp_windows),
        d("crash", "Relatórios de erro e dumps", "Dumps de travamento e relatórios de erro do Windows.", false, 0, crash),
        d("thumbs", "Cache de miniaturas", "Miniaturas e ícones em cache; o Windows recria quando precisa.", false, 0, thumbs),
        d("winupdate", "Cache do Windows Update", "Arquivos de atualização já baixados; são baixados de novo se necessário.", true, 0, windows_update),
        d("browsers", "Cache de navegadores", "Chrome, Edge, Brave e Firefox. Feche os navegadores antes; você continua logado.", false, 0, browsers),
    ]
}

pub fn categories() -> Vec<Category> {
    let mut v: Vec<Category> = defs().into_iter().map(|d| d.cat).collect();
    v.push(Category {
        id: "recycle",
        name: "Lixeira",
        description: "Esvazia a Lixeira. Os arquivos não poderão ser recuperados.",
        admin: false,
    });
    v
}

fn ps(script: &str) -> Result<String, String> {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn recycle(delete: bool) -> (u64, u64) {
    let scan = "$i=@((New-Object -ComObject Shell.Application).NameSpace(10).Items()); \
                $s=($i|ForEach-Object{$_.Size}|Measure-Object -Sum).Sum; \"$($i.Count) $([int64]$s)\"";
    let (files, bytes) = ps(scan)
        .ok()
        .and_then(|o| {
            let mut it = o.split_whitespace();
            Some((it.next()?.parse().ok()?, it.next().and_then(|b| b.parse().ok()).unwrap_or(0)))
        })
        .unwrap_or((0, 0));
    if delete && files > 0 {
        let _ = ps("Clear-RecycleBin -Force -ErrorAction SilentlyContinue");
    }
    (files, bytes)
}

fn run(ids: &[String], delete: bool) -> Vec<Report> {
    let all = defs();
    let admin = crate::tweaks::is_admin();
    ids.iter()
        .filter_map(|id| {
            if id == "recycle" {
                let (files, bytes) = recycle(delete);
                return Some(Report { id: id.clone(), files, bytes });
            }
            let d = all.iter().find(|d| d.cat.id == id.as_str())?;
            if d.cat.admin && !admin {
                return Some(Report { id: id.clone(), files: 0, bytes: 0 });
            }
            let cutoff = SystemTime::now() - Duration::from_secs(d.min_age_hours * 3600);
            let (mut files, mut bytes) = (0, 0);
            for p in (d.paths)() {
                let s = sweep(&p, cutoff, delete);
                files += s.files;
                bytes += s.bytes;
            }
            Some(Report { id: id.clone(), files, bytes })
        })
        .collect()
}

pub fn scan(ids: &[String]) -> Vec<Report> {
    run(ids, false)
}

pub fn clean(ids: &[String]) -> Vec<Report> {
    run(ids, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_unique_ids_and_scan_never_deletes() {
        let cats = categories();
        let mut ids: Vec<_> = cats.iter().map(|c| c.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), cats.len());
        // Varredura de todas as categorias (só leitura) não pode falhar.
        let all: Vec<String> = cats.iter().map(|c| c.id.to_string()).collect();
        assert_eq!(scan(&all).len(), all.len());
        assert!(scan(&["inexistente".to_string()]).is_empty());
    }
}
