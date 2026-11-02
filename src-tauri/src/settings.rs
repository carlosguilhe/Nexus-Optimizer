use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

/// Perfil aplicado enquanto um jogo está aberto. Tudo que o perfil muda é desfeito quando o jogo fecha.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Profile {
    /// High | AboveNormal | Normal
    pub priority: String,
    /// Programas a encerrar quando o jogo abre (ex.: navegador). Nunca processos do sistema.
    pub close_apps: Vec<String>,
    /// Ajustes do catálogo ativados só durante o jogo.
    pub tweaks: Vec<String>,
    /// Máscara de núcleos lógicos permitidos (bit 0 = núcleo 0). None = todos.
    pub affinity: Option<u64>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { priority: "High".into(), close_apps: vec![], tweaks: vec![], affinity: None }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameProfile {
    pub exe: String,
    pub name: String,
    pub profile: Profile,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub auto_game_mode: bool,
    /// Cria um ponto de restauração antes de aplicar ajustes de risco "avançado".
    pub auto_restore: bool,
    pub minimize_to_tray: bool,
    pub notifications: bool,
    /// Limpeza automática a cada N dias (0 = desligada).
    pub clean_every_days: u32,
    pub last_clean: u64,
    pub check_updates: bool,
    pub theme: String,
    pub lang: String,
    pub wizard_done: bool,
    pub default_profile: Profile,
    pub profiles: Vec<GameProfile>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_game_mode: false,
            auto_restore: true,
            minimize_to_tray: false,
            notifications: true,
            clean_every_days: 0,
            last_clean: 0,
            check_updates: true,
            theme: "nexus".into(),
            lang: "pt-BR".into(),
            wizard_done: false,
            default_profile: Profile::default(),
            profiles: vec![],
        }
    }
}

static CELL: OnceLock<RwLock<Settings>> = OnceLock::new();

fn path() -> Result<std::path::PathBuf, String> {
    Ok(crate::tweaks::data_dir()?.join("settings.json"))
}

fn cell() -> &'static RwLock<Settings> {
    CELL.get_or_init(|| {
        let loaded = path()
            .ok()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .unwrap_or_default();
        RwLock::new(loaded)
    })
}

pub fn get() -> Settings {
    cell().read().unwrap().clone()
}

/// Aplica uma mudança e grava em disco.
pub fn update(f: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    let mut guard = cell().write().unwrap();
    f(&mut guard);
    let json = serde_json::to_vec_pretty(&*guard).map_err(|e| e.to_string())?;
    std::fs::write(path()?, json).map_err(|e| e.to_string())?;
    Ok(guard.clone())
}

pub fn replace(new: Settings) -> Result<Settings, String> {
    update(|s| *s = new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_file_still_loads_with_defaults() {
        let s: Settings = serde_json::from_str(r#"{"auto_game_mode":true}"#).unwrap();
        assert!(s.auto_game_mode && s.auto_restore && s.notifications);
        assert_eq!(s.theme, "nexus");
        assert_eq!(s.default_profile.priority, "High");
    }

    #[test]
    fn profile_roundtrip() {
        let p = Profile { priority: "Normal".into(), close_apps: vec!["chrome.exe".into()], tweaks: vec!["x".into()], affinity: Some(0b1111) };
        let back: Profile = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(p, back);
    }
}
