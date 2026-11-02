use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use sysinfo::{ProcessesToUpdate, System};

static SYS: OnceLock<Mutex<System>> = OnceLock::new();

fn sys() -> &'static Mutex<System> {
    SYS.get_or_init(|| Mutex::new(System::new().expect("não foi possível inicializar a leitura do sistema")))
}

#[derive(Serialize)]
pub struct Usage {
    pub cpu_pct: f32,
    pub ram_used: u64,
    pub ram_total: u64,
}

#[derive(Serialize)]
pub struct ProcGroup {
    pub name: String,
    pub count: u32,
    pub mem_bytes: u64,
    pub cpu_pct: f32,
    /// true = processo do sistema ou do próprio app; não pode ser encerrado por aqui.
    pub protected: bool,
}

/// O uso de CPU é calculado entre duas leituras, então a primeira chamada pode vir zerada.
pub fn usage() -> Usage {
    let mut s = sys().lock().unwrap();
    s.refresh_cpu_usage();
    s.refresh_memory();
    Usage { cpu_pct: s.global_cpu_usage(), ram_used: s.used_memory(), ram_total: s.total_memory() }
}

pub fn windows_dir() -> String {
    std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()).to_lowercase()
}

fn is_protected(name: &str, exe: Option<&std::path::Path>) -> bool {
    let n = name.to_lowercase();
    if n == "nexus-optimizer.exe" || n == "system" || n == "registry" {
        return true;
    }
    match exe {
        // Sem caminho legível = processo protegido; dentro de C:\Windows = sistema.
        None => true,
        Some(p) => p.to_string_lossy().to_lowercase().starts_with(&windows_dir()),
    }
}

/// Atualiza a lista de processos e a entrega ao chamador (leitura compartilhada pelo app todo).
pub fn with_processes<R>(f: impl FnOnce(&System) -> R) -> R {
    let mut s = sys().lock().unwrap();
    s.refresh_processes(ProcessesToUpdate::All, true);
    f(&s)
}

pub fn processes(limit: usize) -> Vec<ProcGroup> {
    let mut s = sys().lock().unwrap();
    s.refresh_processes(ProcessesToUpdate::All, true);
    let mut groups: HashMap<String, ProcGroup> = HashMap::new();
    for p in s.processes().values() {
        let name = p.name().to_string_lossy().to_string();
        let protected = is_protected(&name, p.exe());
        let g = groups.entry(name.clone()).or_insert(ProcGroup {
            name,
            count: 0,
            mem_bytes: 0,
            cpu_pct: 0.0,
            protected,
        });
        g.count += 1;
        g.mem_bytes += p.memory();
        g.cpu_pct += p.cpu_usage();
        g.protected |= protected;
    }
    let mut v: Vec<ProcGroup> = groups.into_values().collect();
    v.sort_by(|a, b| b.mem_bytes.cmp(&a.mem_bytes));
    v.truncate(limit);
    v
}

/// Encerra todos os processos com esse nome, exceto os protegidos. Devolve quantos foram encerrados.
pub fn kill(name: &str) -> Result<u32, String> {
    let mut s = sys().lock().unwrap();
    s.refresh_processes(ProcessesToUpdate::All, true);
    let mut killed = 0;
    let mut found = false;
    for p in s.processes().values() {
        let pname = p.name().to_string_lossy().to_string();
        if !pname.eq_ignore_ascii_case(name) {
            continue;
        }
        found = true;
        if is_protected(&pname, p.exe()) {
            return Err(format!("{pname} é um processo do sistema e não pode ser encerrado por aqui."));
        }
        if p.kill() {
            killed += 1;
        }
    }
    if !found {
        return Err("processo não encontrado (já foi encerrado?)".into());
    }
    if killed == 0 {
        return Err("não foi possível encerrar (sem permissão; tente como administrador).".into());
    }
    Ok(killed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn protection_rules() {
        assert!(is_protected("svchost.exe", Some(Path::new(r"C:\Windows\System32\svchost.exe"))));
        assert!(is_protected("whatever.exe", None));
        assert!(is_protected("nexus-optimizer.exe", Some(Path::new(r"D:\x\nexus-optimizer.exe"))));
        assert!(!is_protected("chrome.exe", Some(Path::new(r"C:\Program Files\Google\Chrome\chrome.exe"))));
    }

    #[test]
    fn can_read_usage_and_processes() {
        let u = usage();
        assert!(u.ram_total > 0);
        let p = processes(5);
        assert!(!p.is_empty());
        assert!(p.windows(2).all(|w| w[0].mem_bytes >= w[1].mem_bytes));
        assert!(kill("this-process-does-not-exist.exe").is_err());
    }
}
