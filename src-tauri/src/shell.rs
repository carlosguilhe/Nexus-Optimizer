use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

pub const REPO: &str = "carlosguilhe/Nexus-Optimizer";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ------------------------------------------------------------ notificações

/// Notificação do Windows, respeitando a opção do usuário.
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if crate::settings::get().notifications {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn tr(lang: &str, key: &str) -> &'static str {
    match (lang, key) {
        ("en", "open") => "Open Nexus",
        ("es", "open") => "Abrir Nexus",
        (_, "open") => "Abrir Nexus",
        ("en", "gm") => "Toggle auto Game Mode",
        ("es", "gm") => "Alternar Modo Juego automático",
        (_, "gm") => "Alternar Modo Jogo automático",
        ("en", "ram") => "Free memory",
        ("es", "ram") => "Liberar memoria",
        (_, "ram") => "Liberar memória",
        ("en", "quit") => "Quit",
        ("es", "quit") => "Salir",
        (_, "quit") => "Sair",
        _ => "",
    }
}

/// Liga/desliga o Modo Jogo automático (bandeja e atalho de teclado) e avisa a interface.
pub fn toggle_game_mode(app: &AppHandle) {
    if let Ok(s) = crate::settings::update(|s| s.auto_game_mode = !s.auto_game_mode) {
        let _ = app.emit("settings", &s);
        notify(app, "Nexus Optimizer", if s.auto_game_mode { "Modo Jogo automático ligado." } else { "Modo Jogo automático desligado." });
    }
}

fn quick_free_memory(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let (before, after) = crate::gamemode::free_memory();
        let mb = before.saturating_sub(after) / (1024 * 1024);
        notify(&app, "Nexus Optimizer", &format!("Memória liberada: {mb} MB."));
    });
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let lang = crate::settings::get().lang;
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", tr(&lang, "open"), true, None::<&str>)?,
            &MenuItem::with_id(app, "gm", tr(&lang, "gm"), true, None::<&str>)?,
            &MenuItem::with_id(app, "ram", tr(&lang, "ram"), true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", tr(&lang, "quit"), true, None::<&str>)?,
        ],
    )?;
    let mut b = TrayIconBuilder::with_id("main")
        .tooltip("Nexus Optimizer")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "gm" => toggle_game_mode(app),
            "ram" => quick_free_memory(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

// ---------------------------------------------------------- atalhos globais

/// Ctrl+Alt+G = Modo Jogo automático, Ctrl+Alt+R = liberar memória.
pub fn hotkey_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let m = Modifiers::CONTROL | Modifiers::ALT;
            if shortcut.matches(m, Code::KeyG) {
                toggle_game_mode(app);
            } else if shortcut.matches(m, Code::KeyR) {
                quick_free_memory(app);
            }
        })
        .build()
}

pub fn register_hotkeys(app: &AppHandle) {
    let m = Some(Modifiers::CONTROL | Modifiers::ALT);
    for code in [Code::KeyG, Code::KeyR] {
        // Se outro programa já usa a combinação, só ficamos sem o atalho.
        let _ = app.global_shortcut().register(Shortcut::new(m, code));
    }
}

// ----------------------------------------------------- abrir links e pastas

fn target(key: &str) -> Option<String> {
    Some(
        match key {
            "nvidia" => "https://www.nvidia.com/Download/index.aspx",
            "amd" => "https://www.amd.com/en/support",
            "intel" => "https://www.intel.com/content/www/us/en/download-center/home.html",
            "realtek" => "https://www.realtek.com/Download",
            "releases" => return Some(format!("https://github.com/{REPO}/releases/latest")),
            "repo" => return Some(format!("https://github.com/{REPO}")),
            "display" => "ms-settings:display",
            "winupdate" => "ms-settings:windowsupdate",
            "sysprotect" => "SystemPropertiesProtection",
            _ => return None,
        }
        .to_string(),
    )
}

/// Abre só destinos conhecidos (a interface informa a chave, nunca o endereço).
pub fn open_target(key: &str) -> Result<(), String> {
    let t = target(key).ok_or("destino desconhecido")?;
    Command::new("cmd")
        .args(["/C", "start", "", &t])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Abre um relatório gerado pelo app (só dentro de Documentos\Nexus Reports).
pub fn open_report(path: &str) -> Result<(), String> {
    let home = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
    let dir = std::path::PathBuf::from(home).join("Documents").join("Nexus Reports");
    let file = std::path::Path::new(path).canonicalize().map_err(|_| "arquivo não encontrado")?;
    let dir = dir.canonicalize().map_err(|_| "pasta de relatórios não encontrada")?;
    if !file.starts_with(&dir) {
        return Err("arquivo fora da pasta de relatórios".into());
    }
    Command::new("explorer").arg(file).creation_flags(CREATE_NO_WINDOW).spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn open_reports_folder() -> Result<(), String> {
    let home = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
    let dir = std::path::PathBuf::from(home).join("Documents").join("Nexus Reports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Command::new("explorer").arg(dir).creation_flags(CREATE_NO_WINDOW).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// -------------------------------------------------------------- atualizações

#[derive(Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
}

fn parse_ver(v: &str) -> Vec<u32> {
    v.trim_start_matches(['v', 'V']).split(['.', '-']).map_while(|p| p.parse().ok()).collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    parse_ver(latest) > parse_ver(current)
}

pub async fn check_update() -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("NexusOptimizer")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .send()
        .await
        .map_err(|e| format!("sem acesso ao GitHub: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("nenhuma versão publicada ainda (HTTP {})", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let latest = v["tag_name"].as_str().ok_or("resposta inesperada do GitHub")?.to_string();
    let current = env!("CARGO_PKG_VERSION").to_string();
    Ok(UpdateInfo { newer: is_newer(&latest, &current), latest, current })
}

// ---------------------------------------------------------------- agendador

/// A cada 20 minutos confere se a limpeza automática venceu e, se sim, limpa o que é seguro e comum.
pub fn start_scheduler(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(20 * 60));
        let s = crate::settings::get();
        if s.clean_every_days == 0 {
            continue;
        }
        let due = crate::activity::now().saturating_sub(s.last_clean) >= u64::from(s.clean_every_days) * 86_400;
        if !due {
            continue;
        }
        let mut ids = vec!["temp-user".to_string(), "crash".to_string(), "thumbs".to_string()];
        if crate::tweaks::is_admin() {
            ids.push("temp-windows".into());
        }
        let freed: u64 = crate::cleaner::clean(&ids).iter().map(|r| r.bytes).sum();
        let _ = crate::settings::update(|s| s.last_clean = crate::activity::now());
        crate::activity::log("clean", format!("Limpeza automática: {} MB liberados", freed / (1024 * 1024)));
        notify(&app, "Nexus Optimizer", &format!("Limpeza automática concluída: {} MB liberados.", freed / (1024 * 1024)));
        let _ = app.emit("clean_done", freed);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("v1.1.0", "1.0.0"));
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(!is_newer("v0.9.9", "1.0.0"));
        assert!(is_newer("2.0", "1.9.9"));
    }

    #[test]
    fn only_known_targets_open() {
        assert!(target("nvidia").is_some() && target("releases").unwrap().contains(REPO));
        assert!(target("calc.exe").is_none() && target("https://evil.example").is_none());
        assert!(open_target("nao-existe").is_err());
        assert!(open_report(r"C:\Windows\System32\notepad.exe").is_err());
    }
}
