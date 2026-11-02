use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn powershell(script: &str) -> Result<String, String> {
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; {script}");
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &full])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// ---------------------------------------------------------------- diagnóstico

/// Lê hardware e configuração atual. Só leitura; a interpretação (alertas) é feita na interface.
pub fn system_info() -> Result<serde_json::Value, String> {
    let script = r#"
$ErrorActionPreference='SilentlyContinue'
$o=@{
 cpu=@(Get-CimInstance Win32_Processor|Select-Object Name,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed)
 gpus=@(Get-CimInstance Win32_VideoController|Select-Object Name,DriverVersion,CurrentRefreshRate)
 mem=@(Get-CimInstance Win32_PhysicalMemory|Select-Object Capacity,Speed,ConfiguredClockSpeed,DeviceLocator)
 os=@(Get-CimInstance Win32_OperatingSystem|Select-Object Caption,BuildNumber)
 disks=@(Get-PhysicalDisk|Select-Object FriendlyName,MediaType,Size)
 battery=(@(Get-CimInstance Win32_Battery).Count -gt 0)
 plan=((powercfg /getactivescheme) -join ' ')
}
$o|ConvertTo-Json -Depth 4 -Compress
"#;
    let text = powershell(script)?;
    serde_json::from_str(text.trim()).map_err(|e| format!("não consegui ler o diagnóstico: {e}"))
}

// -------------------------------------------------------------------- limpeza

#[derive(Serialize, Debug, PartialEq)]
pub struct CleanReport {
    pub files: u64,
    pub bytes: u64,
}

/// Percorre `root` sem seguir links. Só considera arquivos modificados antes de `cutoff`
/// (assim não mexe em temporários de programas abertos). Com `delete`, apaga.
pub fn sweep(root: &Path, cutoff: SystemTime, delete: bool) -> CleanReport {
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                stack.push(p.clone());
                dirs.push(p);
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            if !md.modified().map(|m| m < cutoff).unwrap_or(false) {
                continue;
            }
            if delete && std::fs::remove_file(&p).is_err() {
                continue; // em uso ou sem permissão
            }
            files += 1;
            bytes += md.len();
        }
    }
    if delete {
        // Remove só pastas antigas e já vazias (remove_dir falha se houver conteúdo).
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for d in dirs {
            let old = std::fs::metadata(&d)
                .and_then(|m| m.modified())
                .map(|m| m < cutoff)
                .unwrap_or(false);
            if old {
                let _ = std::fs::remove_dir(&d);
            }
        }
    }
    CleanReport { files, bytes }
}

// --------------------------------------------------------------------- prioridade

fn valid_process_name(name: &str) -> Option<String> {
    let n = name.trim().trim_end_matches(".exe").trim_end_matches(".EXE");
    let ok = !n.is_empty()
        && n.len() <= 64
        && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    ok.then(|| n.to_string())
}

/// Define a prioridade de um processo em execução. Não persiste: volta ao normal quando o programa fecha.
pub fn set_priority(process: &str, level: &str) -> Result<u32, String> {
    let name = valid_process_name(process).ok_or("nome de processo inválido")?;
    let level = match level {
        "High" | "AboveNormal" | "Normal" => level,
        _ => return Err("nível de prioridade inválido".into()),
    };
    let script = format!(
        "$ErrorActionPreference='Stop'; $p=@(Get-Process -Name '{name}'); $p|ForEach-Object{{$_.PriorityClass='{level}'}}; $p.Count"
    );
    let out = powershell(&script).map_err(|e| {
        if e.to_lowercase().contains("access") || e.contains("negado") {
            "Acesso negado: abra o app como administrador para alterar esse processo.".to_string()
        } else {
            format!("Processo não encontrado ou não alterável ({name}). O jogo está aberto?")
        }
    })?;
    out.trim().parse::<u32>().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------- ping

#[derive(Serialize, Debug)]
pub struct PingResult {
    pub host: String,
    pub sent: u32,
    pub received: u32,
    pub loss_pct: f64,
    pub min: f64,
    pub avg: f64,
    pub max: f64,
    pub jitter: f64,
}

fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 64
        && !h.starts_with('-')
        && h.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

/// Interpreta a saída do `ping` em qualquer idioma: linhas com "TTL=" são respostas e o tempo vem antes de "ms".
pub fn parse_ping(host: &str, sent: u32, text: &str) -> Result<PingResult, String> {
    let times: Vec<f64> = text
        .lines()
        .filter(|l| l.contains("TTL="))
        .filter_map(|l| {
            let before = &l[..l.find("ms")?];
            let num: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            num.parse::<f64>().ok()
        })
        .collect();
    if times.is_empty() {
        return Err(format!("sem resposta de {host} (100% de perda ou host inválido)"));
    }
    let n = times.len() as f64;
    let jitter = if times.len() > 1 {
        times.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (n - 1.0)
    } else {
        0.0
    };
    Ok(PingResult {
        host: host.to_string(),
        sent,
        received: times.len() as u32,
        loss_pct: (1.0 - n / sent as f64) * 100.0,
        min: times.iter().cloned().fold(f64::MAX, f64::min),
        avg: times.iter().sum::<f64>() / n,
        max: times.iter().cloned().fold(0.0, f64::max),
        jitter,
    })
}

pub fn ping_test(host: &str, count: u32) -> Result<PingResult, String> {
    let host = host.trim();
    if !valid_host(host) {
        return Err("host inválido".into());
    }
    let count = count.clamp(4, 100);
    let out = Command::new("ping")
        .args(["-n", &count.to_string(), host])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    parse_ping(host, count, &String::from_utf8_lossy(&out.stdout))
}

// ------------------------------------------------------- restauração e DNS

/// Cria um ponto de restauração do Windows (exige administrador e Proteção do Sistema ativa).
pub fn restore_point() -> Result<String, String> {
    if !crate::tweaks::is_admin() {
        return Err("Criar ponto de restauração exige administrador.".into());
    }
    let script = r#"
$ErrorActionPreference='Stop'
$before=@(Get-ComputerRestorePoint).Count
Checkpoint-Computer -Description 'Nexus Optimizer' -RestorePointType MODIFY_SETTINGS -WarningAction SilentlyContinue
$after=@(Get-ComputerRestorePoint).Count
if($after -gt $before){'created'}else{'skipped'}
"#;
    let out = powershell(script).map_err(|e| {
        if e.to_lowercase().contains("desativad") || e.to_lowercase().contains("disabled") {
            "A Proteção do Sistema está desativada. Ative em Propriedades do Sistema > Proteção do Sistema.".to_string()
        } else {
            format!("Não foi possível criar o ponto de restauração: {e}")
        }
    })?;
    if out.trim() == "created" {
        Ok("Ponto de restauração criado.".into())
    } else {
        Err("O Windows só cria um ponto a cada 24 horas; já existe um recente.".into())
    }
}

const DNS_PRESETS: &[(&str, &str, &str)] = &[
    ("cloudflare", "1.1.1.1", "1.0.0.1"),
    ("google", "8.8.8.8", "8.8.4.4"),
    ("quad9", "9.9.9.9", "149.112.112.112"),
    ("opendns", "208.67.222.222", "208.67.220.220"),
];

const UP_ADAPTERS: &str = "Get-NetAdapter -Physical | Where-Object Status -eq 'Up'";

/// DNS IPv4 atual dos adaptadores ativos.
pub fn get_dns() -> Result<String, String> {
    let script = format!(
        "$ErrorActionPreference='Stop'; ({UP_ADAPTERS} | ForEach-Object {{ (Get-DnsClientServerAddress -InterfaceIndex $_.ifIndex -AddressFamily IPv4).ServerAddresses }} | Select-Object -Unique) -join ', '"
    );
    powershell(&script).map(|s| s.trim().to_string())
}

/// `preset`: cloudflare | google | quad9 | auto (volta ao DNS automático do roteador).
/// Só valores da lista fixa chegam ao PowerShell.
pub fn set_dns(preset: &str) -> Result<(), String> {
    if !crate::tweaks::is_admin() {
        return Err("Alterar o DNS exige administrador.".into());
    }
    let action = if preset == "auto" {
        "Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ResetServerAddresses".to_string()
    } else {
        let (_, a, b) = DNS_PRESETS.iter().find(|p| p.0 == preset).ok_or("DNS desconhecido")?;
        format!("Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ServerAddresses ('{a}','{b}')")
    };
    powershell(&format!("$ErrorActionPreference='Stop'; {UP_ADAPTERS} | ForEach-Object {{ {action} }}"))
        .map(|_| ())
        .map_err(|e| format!("Falha ao alterar o DNS: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const MIN_AGE: Duration = Duration::from_secs(24 * 3600);

    #[test]
    fn dns_presets_reject_unknown() {
        assert!(set_dns("'; calc; '").is_err());
    }

    #[test]
    fn ping_parsing_is_language_neutral() {
        let pt = "Resposta de 1.1.1.1: bytes=32 tempo=20ms TTL=58\n\
                  Esgotado o tempo limite do pedido.\n\
                  Resposta de 1.1.1.1: bytes=32 tempo<1ms TTL=58\n\
                  Resposta de 1.1.1.1: bytes=32 tempo=30ms TTL=58\n";
        let r = parse_ping("1.1.1.1", 4, pt).unwrap();
        assert_eq!(r.received, 3);
        assert_eq!(r.loss_pct, 25.0);
        assert_eq!((r.min, r.max), (1.0, 30.0));
        assert!((r.avg - 17.0).abs() < 1e-9);
        assert!((r.jitter - 24.0).abs() < 1e-9); // (|1-20| + |30-1|) / 2
        assert!(parse_ping("x", 4, "Esgotado o tempo limite").is_err());
    }

    #[test]
    fn input_validation() {
        assert!(valid_host("1.1.1.1") && valid_host("google.com"));
        assert!(!valid_host("-n 1000") && !valid_host("a b") && !valid_host("a;b") && !valid_host(""));
        assert_eq!(valid_process_name("VALORANT-Win64-Shipping.exe").unwrap(), "VALORANT-Win64-Shipping");
        assert!(valid_process_name("x'; Remove-Item *").is_none());
        assert!(set_priority("game", "RealTime").is_err());
    }

    #[test]
    fn sweep_only_touches_old_files() {
        let root = std::env::temp_dir().join("nexus_sweep_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let old = root.join("sub").join("old.tmp");
        let new = root.join("new.tmp");
        std::fs::write(&old, vec![0u8; 1000]).unwrap();
        std::fs::write(&new, vec![0u8; 10]).unwrap();
        let three_days = SystemTime::now() - Duration::from_secs(3 * 24 * 3600);
        std::fs::File::options().write(true).open(&old).unwrap().set_modified(three_days).unwrap();
        std::fs::File::options().write(true).open(root.join("sub")).ok();

        let cutoff = SystemTime::now() - MIN_AGE;
        assert_eq!(sweep(&root, cutoff, false), CleanReport { files: 1, bytes: 1000 });
        assert!(old.exists(), "scan não pode apagar");
        assert_eq!(sweep(&root, cutoff, true), CleanReport { files: 1, bytes: 1000 });
        assert!(!old.exists() && new.exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
