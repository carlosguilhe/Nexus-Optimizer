use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Apps pré-instalados que quase ninguém usa. Lista fechada: só estes nomes chegam ao PowerShell.
/// Todos podem ser reinstalados pela Microsoft Store.
const REMOVABLE: &[(&str, &str)] = &[
    ("Microsoft.BingNews", "Notícias"),
    ("Microsoft.BingWeather", "Clima"),
    ("Microsoft.GetHelp", "Obter Ajuda"),
    ("Microsoft.Getstarted", "Dicas"),
    ("Microsoft.MicrosoftSolitaireCollection", "Microsoft Solitaire Collection"),
    ("Microsoft.WindowsFeedbackHub", "Hub de Comentários"),
    ("Microsoft.WindowsMaps", "Mapas"),
    ("Microsoft.ZuneVideo", "Filmes e TV"),
    ("Microsoft.People", "Pessoas"),
    ("Microsoft.SkypeApp", "Skype"),
    ("Microsoft.Microsoft3DViewer", "Visualizador 3D"),
    ("Microsoft.Paint3D", "Paint 3D"),
    ("Microsoft.MixedReality.Portal", "Portal de Realidade Misturada"),
    ("Microsoft.MicrosoftOfficeHub", "Microsoft 365 (Office)"),
    ("Microsoft.PowerAutomateDesktop", "Power Automate"),
    ("Microsoft.YourPhone", "Link para Celular"),
    ("Microsoft.Todos", "Microsoft To Do"),
    ("Clipchamp.Clipchamp", "Clipchamp"),
    ("MicrosoftTeams", "Microsoft Teams (pessoal)"),
    ("SpotifyAB.SpotifyMusic", "Spotify"),
    ("Disney.37853FC22B2CE", "Disney+"),
    ("AmazonVideo.PrimeVideo", "Prime Video"),
    ("BytedancePte.Ltd.TikTok", "TikTok"),
    ("Facebook.Facebook", "Facebook"),
];

#[derive(Serialize)]
pub struct App {
    pub package: &'static str,
    pub label: &'static str,
}

#[derive(Deserialize)]
struct Found {
    #[serde(rename = "Name")]
    name: String,
}

fn ps(script: &str) -> Result<String, String> {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Quais apps da lista estão instalados para o usuário atual.
pub fn installed() -> Result<Vec<App>, String> {
    let names = REMOVABLE.iter().map(|(n, _)| format!("'{n}'")).collect::<Vec<_>>().join(",");
    let out = ps(&format!(
        "$ErrorActionPreference='SilentlyContinue'; @(Get-AppxPackage -Name {names} | Select-Object Name) | ConvertTo-Json -Compress"
    ))?;
    let text = out.trim();
    if text.is_empty() {
        return Ok(vec![]);
    }
    // ConvertTo-Json devolve um objeto (não lista) quando há um só item.
    let found: Vec<Found> = if text.starts_with('[') {
        serde_json::from_str(text).map_err(|e| e.to_string())?
    } else {
        vec![serde_json::from_str(text).map_err(|e| e.to_string())?]
    };
    Ok(REMOVABLE
        .iter()
        .filter(|(n, _)| found.iter().any(|f| f.name.eq_ignore_ascii_case(n)))
        .map(|(package, label)| App { package, label })
        .collect())
}

pub fn remove(package: &str) -> Result<(), String> {
    let (name, _) = REMOVABLE.iter().find(|(n, _)| *n == package).ok_or("app fora da lista de remoção")?;
    ps(&format!("$ErrorActionPreference='Stop'; Get-AppxPackage -Name '{name}' | Remove-AppxPackage"))
        .map(|_| ())
        .map_err(|e| format!("Não foi possível remover: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_packages_are_removable() {
        assert!(remove("Microsoft.WindowsStore").is_err());
        assert!(remove("x'; Remove-Item C:\\ -Recurse; '").is_err());
        let mut names: Vec<_> = REMOVABLE.iter().map(|(n, _)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), REMOVABLE.len());
    }

    #[test]
    fn can_list_installed() {
        assert!(installed().is_ok());
    }
}
