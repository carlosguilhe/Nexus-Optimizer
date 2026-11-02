use serde::{Deserialize, Serialize};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LINES: usize = 500;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub ts: u64,
    /// apply | revert | clean | app | startup | dns | restore | profile | system
    pub kind: String,
    pub text: String,
}

fn path() -> Result<std::path::PathBuf, String> {
    Ok(crate::tweaks::data_dir()?.join("activity.jsonl"))
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Registra algo que o Nexus mudou no PC. Falhas ao gravar o log nunca atrapalham a ação.
pub fn log(kind: &str, text: impl Into<String>) {
    let entry = Entry { ts: now(), kind: kind.into(), text: text.into() };
    let Ok(p) = path() else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        if let Ok(line) = serde_json::to_string(&entry) {
            let _ = writeln!(f, "{line}");
        }
    }
    trim(&p);
}

fn trim(p: &std::path::Path) {
    let Ok(text) = std::fs::read_to_string(p) else { return };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_LINES + 100 {
        let keep = lines[lines.len() - MAX_LINES..].join("\n") + "\n";
        let _ = std::fs::write(p, keep);
    }
}

/// Mais recentes primeiro.
pub fn list(limit: usize) -> Vec<Entry> {
    let Ok(p) = path() else { return vec![] };
    let text = std::fs::read_to_string(p).unwrap_or_default();
    let mut v: Vec<Entry> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    v.reverse();
    v.truncate(limit);
    v
}

pub fn clear() -> Result<(), String> {
    match std::fs::remove_file(path()?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_list_clear() {
        // SAFETY: teste isolado usa uma pasta de dados temporária.
        unsafe { std::env::set_var("NEXUS_DATA_DIR", std::env::temp_dir().join("nexus_test_data")) };
        clear().unwrap();
        log("apply", "um");
        log("revert", "dois");
        let v = list(10);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].text, "dois", "mais recente primeiro");
        clear().unwrap();
        assert!(list(10).is_empty());
    }
}
