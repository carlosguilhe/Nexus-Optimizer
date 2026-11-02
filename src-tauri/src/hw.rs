use serde::Serialize;
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    GetProcessAffinityMask, OpenProcess, SetPriorityClass, SetProcessAffinityMask, ABOVE_NORMAL_PRIORITY_CLASS,
    HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn powershell(script: &str) -> Result<String, String> {
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; $ErrorActionPreference='SilentlyContinue'; {script}");
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &full])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn json(script: &str) -> Result<serde_json::Value, String> {
    let text = powershell(script)?;
    serde_json::from_str(text.trim()).map_err(|e| format!("resposta inesperada do Windows: {e}"))
}

// ------------------------------------------------------- prioridade e núcleos

pub fn priority_class(level: &str) -> u32 {
    match level {
        "High" => HIGH_PRIORITY_CLASS,
        "AboveNormal" => ABOVE_NORMAL_PRIORITY_CLASS,
        _ => NORMAL_PRIORITY_CLASS,
    }
}

pub fn set_priority(pid: u32, level: &str) -> bool {
    // SAFETY: handle aberto aqui e fechado logo depois; nada é retido.
    unsafe {
        let h = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let ok = SetPriorityClass(h, priority_class(level)) != 0;
        CloseHandle(h);
        ok
    }
}

pub fn cpu_count() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

/// Restringe o processo aos núcleos da máscara (bit 0 = núcleo 0), limitada aos núcleos que existem.
pub fn set_affinity(pid: u32, mask: u64) -> bool {
    // SAFETY: igual a set_priority; os ponteiros apontam para locais válidos durante a chamada.
    unsafe {
        let h = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let (mut proc_mask, mut sys_mask) = (0usize, 0usize);
        let mut ok = false;
        if GetProcessAffinityMask(h, &mut proc_mask, &mut sys_mask) != 0 {
            let want = (mask as usize) & sys_mask;
            ok = want != 0 && SetProcessAffinityMask(h, want) != 0;
        }
        CloseHandle(h);
        ok
    }
}

// ------------------------------------------------------------------ GPU

#[derive(Serialize, Debug, PartialEq)]
pub struct Gpu {
    pub name: String,
    pub temp_c: Option<f64>,
    pub util_pct: Option<f64>,
    pub mem_used_mb: Option<f64>,
    pub mem_total_mb: Option<f64>,
    pub power_w: Option<f64>,
    pub clock_mhz: Option<f64>,
    pub fan_pct: Option<f64>,
}

pub fn parse_gpu_csv(text: &str) -> Vec<Gpu> {
    let num = |s: Option<&str>| s.and_then(|v| v.trim().parse::<f64>().ok());
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Gpu {
                name: f.first().map(|s| s.trim().to_string()).unwrap_or_default(),
                temp_c: num(f.get(1).copied()),
                util_pct: num(f.get(2).copied()),
                mem_used_mb: num(f.get(3).copied()),
                mem_total_mb: num(f.get(4).copied()),
                power_w: num(f.get(5).copied()),
                clock_mhz: num(f.get(6).copied()),
                fan_pct: num(f.get(7).copied()),
            }
        })
        .collect()
}

/// Lê a GPU NVIDIA pelo nvidia-smi (que acompanha o driver). Sem NVIDIA, devolve um erro explicando.
pub fn gpu_stats() -> Result<Vec<Gpu>, String> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw,clocks.gr,fan.speed",
            "--format=csv,noheader,nounits",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|_| "nvidia-smi não encontrado: o monitor de GPU funciona só com placas NVIDIA e o driver instalado.".to_string())?;
    if !out.status.success() {
        return Err("O nvidia-smi não respondeu. Atualize o driver da NVIDIA.".into());
    }
    Ok(parse_gpu_csv(&String::from_utf8_lossy(&out.stdout)))
}

// --------------------------------------------------------- disco e drivers

pub fn disk_health() -> Result<serde_json::Value, String> {
    json(
        "@{ disks=@(Get-PhysicalDisk|Select-Object FriendlyName,MediaType,HealthStatus,OperationalStatus,Size); \
         volumes=@(Get-Volume|Where-Object DriveType -eq 'Fixed'|Select-Object DriveLetter,FileSystemLabel,Size,SizeRemaining); \
         trim=((fsutil behavior query DisableDeleteNotify) -join ' ') } | ConvertTo-Json -Depth 3 -Compress",
    )
}

pub fn drivers() -> Result<serde_json::Value, String> {
    json(
        "@(Get-CimInstance Win32_PnPSignedDriver | Where-Object { $_.DeviceClass -in 'DISPLAY','NET','MEDIA' -and $_.DeviceName } | \
         Select-Object DeviceName,DriverVersion,Manufacturer,DeviceClass,@{n='DriverDate';e={if($_.DriverDate){$_.DriverDate.ToString('yyyy-MM-dd')}else{''}}}) | ConvertTo-Json -Depth 3 -Compress",
    )
}

// --------------------------------------------------------------- MSI mode

const PCI: &str = r"SYSTEM\CurrentControlSet\Enum\PCI";
const MSI_SUB: &str = r"Device Parameters\Interrupt Management\MessageSignaledInterruptProperties";

#[derive(Serialize, Debug)]
pub struct MsiDevice {
    pub id: String,
    pub name: String,
    pub class: String,
    /// Estado atual de MSISupported (None = valor ausente).
    pub msi: Option<bool>,
    pub changed: bool,
}

fn msi_backup_path() -> Result<std::path::PathBuf, String> {
    Ok(crate::tweaks::data_dir()?.join("msi_backup.json"))
}

fn msi_backup() -> HashMap<String, Option<u32>> {
    msi_backup_path()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_msi_backup(b: &HashMap<String, Option<u32>>) -> Result<(), String> {
    std::fs::write(msi_backup_path()?, serde_json::to_vec_pretty(b).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// "@oem12.inf,%name%;NVIDIA GeForce GTX 1650" -> "NVIDIA GeForce GTX 1650"
fn clean_desc(raw: &str) -> String {
    raw.rsplit(';').next().unwrap_or(raw).trim().to_string()
}

/// Placas de vídeo e de rede PCI e o estado do MSI de cada uma.
pub fn msi_list() -> Vec<MsiDevice> {
    let backup = msi_backup();
    let Ok(pci) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(PCI) else { return vec![] };
    let mut out = Vec::new();
    for ven in pci.enum_keys().flatten() {
        let Ok(vk) = pci.open_subkey(&ven) else { continue };
        for inst in vk.enum_keys().flatten() {
            let Ok(ik) = vk.open_subkey(&inst) else { continue };
            let class: String = ik.get_value("Class").unwrap_or_default();
            if !matches!(class.as_str(), "Display" | "Net") {
                continue;
            }
            let name = ik
                .get_value::<String, _>("FriendlyName")
                .or_else(|_| ik.get_value::<String, _>("DeviceDesc").map(|d| clean_desc(&d)))
                .unwrap_or_else(|_| ven.clone());
            let id = format!(r"{ven}\{inst}");
            let msi = ik
                .open_subkey(MSI_SUB)
                .ok()
                .and_then(|k| k.get_value::<u32, _>("MSISupported").ok())
                .map(|v| v == 1);
            out.push(MsiDevice { changed: backup.contains_key(&id), id, name, class, msi });
        }
    }
    out
}

fn msi_path(id: &str) -> Option<String> {
    // O id vem da lista acima; ainda assim só aceita VEN_xxx\instância, sem ".." nem barras extras.
    let mut parts = id.split('\\');
    let (v, i) = (parts.next()?, parts.next()?);
    (parts.next().is_none() && v.starts_with("VEN_") && !i.is_empty() && !id.contains("..")).then(|| format!(r"{PCI}\{v}\{i}\{MSI_SUB}"))
}

pub fn msi_set(id: &str, on: bool) -> Result<(), String> {
    if !crate::tweaks::is_admin() {
        return Err("Alterar o MSI Mode exige administrador.".into());
    }
    let path = msi_path(id).ok_or("dispositivo inválido")?;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut backup = msi_backup();
    if on {
        let original = hklm.open_subkey(&path).ok().and_then(|k| k.get_value::<u32, _>("MSISupported").ok());
        backup.entry(id.to_string()).or_insert(original);
        save_msi_backup(&backup)?; // backup antes de mexer
        let (key, _) = hklm.create_subkey(&path).map_err(|e| e.to_string())?;
        key.set_value("MSISupported", &1u32).map_err(|e| e.to_string())
    } else {
        let original = backup.get(id).copied().ok_or("sem backup para este dispositivo; nada a reverter")?;
        if let Ok(key) = hklm.open_subkey_with_flags(&path, KEY_SET_VALUE) {
            match original {
                Some(v) => key.set_value("MSISupported", &v).map_err(|e| e.to_string())?,
                None => {
                    let _ = key.delete_value("MSISupported");
                }
            }
        }
        backup.remove(id);
        save_msi_backup(&backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_csv_parses_values_and_na() {
        let g = parse_gpu_csv("NVIDIA GeForce GTX 1650, 45, 12, 512, 4096, 20.5, 1485, [N/A]\n");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "NVIDIA GeForce GTX 1650");
        assert_eq!((g[0].temp_c, g[0].mem_total_mb, g[0].power_w), (Some(45.0), Some(4096.0), Some(20.5)));
        assert_eq!(g[0].fan_pct, None);
    }

    #[test]
    fn msi_ids_are_validated() {
        assert!(msi_path(r"VEN_10DE&DEV_1F82&SUBSYS_1\4&2f1d").is_some());
        assert!(msi_path(r"..\..\x").is_none());
        assert!(msi_path(r"VEN_1\a\b").is_none());
        assert_eq!(clean_desc("@oem1.inf,%x%;NVIDIA GeForce"), "NVIDIA GeForce");
    }

    #[test]
    fn can_list_msi_and_cpus_without_admin() {
        let _ = msi_list();
        assert!(cpu_count() >= 1);
        assert!(msi_set("VEN_x\\y", true).is_err() || crate::tweaks::is_admin());
    }

    #[test]
    fn priority_classes_map() {
        assert_eq!(priority_class("High"), HIGH_PRIORITY_CLASS);
        assert_eq!(priority_class("qualquer"), NORMAL_PRIORITY_CLASS);
    }
}
