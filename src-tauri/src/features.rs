use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Recursos opcionais do Windows que o app liga e desliga. Lista fechada: só estes nomes chegam ao PowerShell.
/// (nome do recurso, rótulo, descrição, aviso)
const FEATURES: &[(&str, &str, &str, &str)] = &[
    (
        "NetFx3",
        ".NET Framework 3.5",
        "Necessário para jogos e programas antigos. A instalação baixa arquivos do Windows Update.",
        "",
    ),
    (
        "DirectPlay",
        "DirectPlay",
        "Componente legado de jogos antigos (anos 2000) que pedem DirectPlay.",
        "",
    ),
    (
        "LegacyComponents",
        "Componentes legados",
        "Dá suporte a programas muito antigos do Windows.",
        "",
    ),
    (
        "Microsoft-Windows-Subsystem-Linux",
        "Subsistema do Windows para Linux (WSL)",
        "Roda distribuições Linux dentro do Windows.",
        "",
    ),
    (
        "Containers-DisposableClientVM",
        "Windows Sandbox",
        "Uma área descartável para testar programas sem risco. Exige Windows Pro e virtualização.",
        "Usa a plataforma de virtualização do Windows, que pode conflitar com alguns anticheats e reduzir um pouco o FPS.",
    ),
    (
        "VirtualMachinePlatform",
        "Plataforma de Máquina Virtual",
        "Base de WSL 2 e emuladores Android.",
        "Ativa o hipervisor do Windows, que pode conflitar com alguns anticheats e reduzir um pouco o FPS.",
    ),
    (
        "Microsoft-Hyper-V-All",
        "Hyper-V",
        "Máquinas virtuais do Windows. Exige Windows Pro.",
        "Ativa o hipervisor do Windows, que pode conflitar com alguns anticheats e reduzir um pouco o FPS.",
    ),
    (
        "TelnetClient",
        "Cliente Telnet",
        "Cliente de linha de comando para testar portas e serviços de rede.",
        "",
    ),
];

#[derive(Serialize)]
pub struct Feature {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub warning: &'static str,
    /// Enabled | Disabled | EnablePending | DisablePending | Unavailable
    pub state: String,
}

#[derive(Deserialize)]
struct Row {
    #[serde(rename = "FeatureName")]
    name: String,
    #[serde(rename = "State")]
    state: String,
}

fn ps(script: &str) -> Result<String, String> {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "o PowerShell falhou".into() } else { err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn parse_rows(text: &str) -> Vec<Row> {
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }
    // ConvertTo-Json devolve um objeto (não lista) quando há um só item.
    if text.starts_with('[') {
        serde_json::from_str(text).unwrap_or_default()
    } else {
        serde_json::from_str(text).map(|r| vec![r]).unwrap_or_default()
    }
}

pub fn list() -> Result<Vec<Feature>, String> {
    if !crate::tweaks::is_admin() {
        return Err("Ler os recursos do Windows exige administrador. Abra o app como administrador.".into());
    }
    let names = FEATURES.iter().map(|f| format!("'{}'", f.0)).collect::<Vec<_>>().join(",");
    // State vem como número no JSON; ToString() devolve o nome (Enabled, Disabled…).
    let out = ps(&format!(
        "$ErrorActionPreference='SilentlyContinue'; @(Get-WindowsOptionalFeature -Online -FeatureName {names} | \
         ForEach-Object {{ [pscustomobject]@{{ FeatureName = $_.FeatureName; State = $_.State.ToString() }} }}) | ConvertTo-Json -Compress"
    ))?;
    let rows = parse_rows(&out);
    Ok(FEATURES
        .iter()
        .map(|(id, label, description, warning)| Feature {
            id,
            label,
            description,
            warning,
            state: rows
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(id))
                .map(|r| r.state.clone())
                .unwrap_or_else(|| "Unavailable".into()),
        })
        .collect())
}

pub fn label_of(id: &str) -> String {
    FEATURES.iter().find(|f| f.0 == id).map(|f| f.1.to_string()).unwrap_or_else(|| id.to_string())
}

/// Liga ou desliga um recurso. Devolve true se o Windows pede reinício para concluir.
pub fn set(id: &str, enable: bool) -> Result<bool, String> {
    let name = FEATURES.iter().find(|f| f.0 == id).map(|f| f.0).ok_or("recurso fora da lista")?;
    if !crate::tweaks::is_admin() {
        return Err("Alterar recursos do Windows exige administrador. Abra o app como administrador.".into());
    }
    let verb = if enable { "Enable-WindowsOptionalFeature -All" } else { "Disable-WindowsOptionalFeature" };
    let out = ps(&format!(
        "$ErrorActionPreference='Stop'; $r = {verb} -Online -FeatureName '{name}' -NoRestart; [string]$r.RestartNeeded"
    ))
    .map_err(|e| format!("Não foi possível alterar o recurso: {e}"))?;
    Ok(out.trim().eq_ignore_ascii_case("true"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_features_can_change() {
        assert!(set("Nao-Existe", true).is_err());
        assert!(set("NetFx3'; calc; '", true).is_err());
    }

    #[test]
    fn ids_are_unique_and_plain() {
        let mut ids: Vec<_> = FEATURES.iter().map(|f| f.0).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert!(ids.iter().all(|i| i.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')));
    }

    #[test]
    fn parses_one_or_many_rows() {
        assert_eq!(parse_rows(r#"{"FeatureName":"NetFx3","State":"Enabled"}"#).len(), 1);
        assert_eq!(parse_rows(r#"[{"FeatureName":"A","State":"Enabled"},{"FeatureName":"B","State":"Disabled"}]"#).len(), 2);
        assert!(parse_rows("").is_empty());
    }
}
