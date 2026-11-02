mod activity;
mod apps;
mod bench;
mod cleaner;
mod gamemode;
mod games;
mod history;
mod hw;
mod maint;
mod net;
mod settings;
mod shell;
mod speedtest;
mod startup;
mod sys;
mod tools;
mod tweaks;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Roda trabalho bloqueante (registro, powercfg, PowerShell) fora da thread da interface.
async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Registra no histórico de atividades quando a ação deu certo.
fn logged<T>(r: Result<T, String>, kind: &str, text: impl FnOnce() -> String) -> Result<T, String> {
    if r.is_ok() {
        activity::log(kind, text());
    }
    r
}

// ------------------------------------------------------------ sistema e janela

/// Reabre o app pedindo elevação (UAC). Só fecha este processo se o novo foi iniciado.
#[tauri::command]
async fn relaunch_admin(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = format!("Start-Process -FilePath '{}' -Verb RunAs", exe.display().to_string().replace('\'', "''"));
    let out = blocking(move || {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &format!("$ErrorActionPreference='Stop'; {script}")])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|e| e.to_string())
    })
    .await?;
    if !out.status.success() {
        return Err("Elevação cancelada ou negada.".into());
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn is_admin() -> bool {
    tweaks::is_admin()
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn autostart_get() -> bool {
    startup::autostart_enabled()
}

#[tauri::command]
fn autostart_set(enabled: bool) -> Result<(), String> {
    logged(startup::set_autostart(enabled), "startup", || {
        format!("Iniciar com o Windows: {}", if enabled { "ligado" } else { "desligado" })
    })
}

#[tauri::command]
fn settings_get() -> settings::Settings {
    settings::get()
}

#[tauri::command]
fn settings_set(app: tauri::AppHandle, value: settings::Settings) -> Result<settings::Settings, String> {
    let s = settings::replace(value)?;
    let _ = app.emit("settings", &s);
    Ok(s)
}

#[tauri::command]
fn activity_list(limit: usize) -> Vec<activity::Entry> {
    activity::list(limit.clamp(1, 500))
}

#[tauri::command]
fn activity_clear() -> Result<(), String> {
    activity::clear()
}

#[tauri::command]
fn open_target(key: String) -> Result<(), String> {
    shell::open_target(&key)
}

#[tauri::command]
fn open_report(path: String) -> Result<(), String> {
    shell::open_report(&path)
}

#[tauri::command]
fn open_reports_folder() -> Result<(), String> {
    shell::open_reports_folder()
}

#[tauri::command]
async fn check_update() -> Result<shell::UpdateInfo, String> {
    shell::check_update().await
}

// --------------------------------------------------------------------- ajustes

#[tauri::command]
async fn list_tweaks() -> Result<Vec<tweaks::TweakInfo>, String> {
    blocking(tweaks::list).await
}

#[tauri::command]
async fn apply_tweak(id: String) -> Result<(), String> {
    blocking(move || {
        // Ajustes avançados: cria um ponto de restauração antes, se o usuário deixou isso ligado.
        if tweaks::risk_of(&id) == Some("avançado")
            && settings::get().auto_restore
            && tweaks::is_admin()
            && tools::restore_point().is_ok()
        {
            activity::log("restore", format!("Ponto de restauração automático antes de: {}", tweaks::name_of(&id)));
        }
        logged(tweaks::apply(&id), "apply", || format!("Aplicado: {}", tweaks::name_of(&id)))
    })
    .await
}

#[tauri::command]
async fn revert_tweak(id: String) -> Result<(), String> {
    blocking(move || logged(tweaks::revert(&id), "revert", || format!("Revertido: {}", tweaks::name_of(&id)))).await
}

// ------------------------------------------------------------- benchmark

#[tauri::command]
async fn run_benchmark(process: String, game: String, label: String, seconds: u32) -> Result<history::Entry, String> {
    blocking(move || {
        let result = bench::run(&process, seconds.clamp(5, 300))?;
        history::add(&game, &process, &label, result)
    })
    .await
}

#[tauri::command]
fn bench_history() -> Vec<history::Entry> {
    history::list()
}

#[tauri::command]
fn bench_delete(id: u64) -> Result<(), String> {
    history::delete(id)
}

#[tauri::command]
fn bench_clear() -> Result<(), String> {
    history::clear()
}

#[tauri::command]
fn ab_verdict(before: Vec<bench::BenchResult>, after: Vec<bench::BenchResult>) -> history::Verdict {
    history::verdict(&before, &after)
}

/// Gera o relatório HTML das medições escolhidas e devolve o caminho do arquivo.
#[tauri::command]
fn bench_report(ids: Vec<u64>) -> Result<String, String> {
    let mut chosen: Vec<history::Entry> = history::list().into_iter().filter(|e| ids.contains(&e.id)).collect();
    if chosen.is_empty() {
        return Err("escolha ao menos uma medição".into());
    }
    chosen.sort_by_key(|e| std::cmp::Reverse(e.id)); // da mais recente para a mais antiga
    let path = history::write_report(&chosen)?;
    activity::log("system", "Relatório de benchmark gerado");
    Ok(path.to_string_lossy().to_string())
}

// --------------------------------------------------------------------- jogos

#[tauri::command]
async fn catalog_status(refresh: bool) -> Result<games::CatalogStatus, String> {
    let cat = games::ensure(refresh).await;
    Ok(games::status(&cat))
}

#[tauri::command]
async fn running_games() -> Result<Vec<games::RunningGame>, String> {
    games::ensure(false).await;
    blocking(|| Ok(games::running_games())).await
}

/// Busca jogos do catálogo pelo nome (para criar perfis de jogos que não estão abertos).
#[tauri::command]
async fn search_games(query: String) -> Result<Vec<games::GameHit>, String> {
    games::ensure(false).await;
    blocking(move || Ok(games::search(&query, 12))).await
}

#[tauri::command]
fn gamemode_get() -> serde_json::Value {
    let st = gamemode::status();
    serde_json::json!({ "enabled": gamemode::is_enabled(), "active": st.active, "game": st.game })
}

#[tauri::command]
fn gamemode_set(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    gamemode::set_enabled(enabled)?;
    let _ = app.emit("settings", settings::get());
    Ok(())
}

#[tauri::command]
async fn free_memory() -> Result<(u64, u64), String> {
    blocking(|| {
        let r = gamemode::free_memory();
        activity::log("system", "Memória liberada manualmente");
        Ok(r)
    })
    .await
}

#[tauri::command]
fn cpu_count() -> usize {
    hw::cpu_count()
}

#[tauri::command]
async fn set_affinity_by_name(process: String, mask: u64) -> Result<u32, String> {
    blocking(move || {
        let name = process.to_lowercase();
        let pids: Vec<u32> = sys::with_processes(|s| {
            s.processes()
                .iter()
                .filter(|(_, p)| p.name().to_string_lossy().to_lowercase() == name)
                .map(|(pid, _)| pid.as_u32())
                .collect()
        });
        let n = pids.iter().filter(|p| hw::set_affinity(**p, mask)).count() as u32;
        if n == 0 {
            return Err("Processo não encontrado ou não alterável. O jogo está aberto? Se ele roda como administrador, abra o Nexus também como administrador.".into());
        }
        activity::log("profile", format!("Afinidade de {process} definida ({n} processo(s))"));
        Ok(n)
    })
    .await
}

// ------------------------------------------------------------ rede e hardware

#[tauri::command]
fn server_list() -> Vec<net::Server> {
    net::SERVERS.to_vec()
}

#[tauri::command]
async fn ping_servers(rounds: usize) -> Vec<net::ServerPing> {
    net::ping_servers(rounds.clamp(1, 8)).await
}

#[tauri::command]
async fn tcp_ping(host: String, port: u16) -> Option<f64> {
    net::tcp_ping(&host, port).await
}

#[tauri::command]
async fn dns_benchmark() -> Result<Vec<net::DnsResult>, String> {
    let current = blocking(tools::get_dns).await.ok().and_then(|s| s.split(',').next().map(|x| x.trim().to_string()));
    Ok(net::dns_benchmark(current).await)
}

#[tauri::command]
async fn route_trace(app: tauri::AppHandle, host: String) -> Result<Vec<net::Hop>, String> {
    blocking(move || net::route_trace(&app, &host)).await
}

#[tauri::command]
async fn gpu_stats() -> Result<Vec<hw::Gpu>, String> {
    blocking(hw::gpu_stats).await
}

#[tauri::command]
async fn disk_health() -> Result<serde_json::Value, String> {
    blocking(hw::disk_health).await
}

#[tauri::command]
async fn drivers() -> Result<serde_json::Value, String> {
    blocking(hw::drivers).await
}

#[tauri::command]
async fn msi_list() -> Result<Vec<hw::MsiDevice>, String> {
    blocking(|| Ok(hw::msi_list())).await
}

#[tauri::command]
async fn msi_set(id: String, enabled: bool) -> Result<(), String> {
    blocking(move || {
        logged(hw::msi_set(&id, enabled), "apply", || {
            format!("MSI Mode {}: {id}", if enabled { "ativado" } else { "revertido" })
        })
    })
    .await
}

#[tauri::command]
async fn speed_test(app: tauri::AppHandle) -> Result<speedtest::SpeedResult, String> {
    speedtest::run(app).await
}

#[tauri::command]
async fn ping_test(host: String, count: u32) -> Result<tools::PingResult, String> {
    blocking(move || tools::ping_test(&host, count)).await
}

#[tauri::command]
async fn get_dns() -> Result<String, String> {
    blocking(tools::get_dns).await
}

#[tauri::command]
async fn set_dns(preset: String) -> Result<(), String> {
    blocking(move || logged(tools::set_dns(&preset), "dns", || format!("DNS alterado para: {preset}"))).await
}

#[tauri::command]
async fn restore_point() -> Result<String, String> {
    blocking(|| logged(tools::restore_point(), "restore", || "Ponto de restauração criado".to_string())).await
}

#[tauri::command]
async fn set_priority(process: String, level: String) -> Result<u32, String> {
    blocking(move || tools::set_priority(&process, &level)).await
}

// ----------------------------------------------------- processos e inicialização

#[tauri::command]
async fn usage() -> sys::Usage {
    blocking(|| Ok(sys::usage())).await.unwrap_or(sys::Usage { cpu_pct: 0.0, ram_used: 0, ram_total: 0 })
}

#[tauri::command]
async fn processes() -> Result<Vec<sys::ProcGroup>, String> {
    blocking(|| Ok(sys::processes(25))).await
}

#[tauri::command]
async fn kill_process(name: String) -> Result<u32, String> {
    blocking(move || logged(sys::kill(&name), "system", || format!("Processo encerrado: {name}"))).await
}

#[tauri::command]
async fn startup_list() -> Result<Vec<startup::StartupItem>, String> {
    blocking(|| Ok(startup::list())).await
}

#[tauri::command]
async fn startup_set(loc: usize, name: String, enabled: bool) -> Result<(), String> {
    blocking(move || {
        logged(startup::set_enabled(loc, &name, enabled), "startup", || {
            format!("Inicialização: {name} {}", if enabled { "ativado" } else { "desativado" })
        })
    })
    .await
}

#[tauri::command]
async fn system_info() -> Result<serde_json::Value, String> {
    blocking(tools::system_info).await
}

// ------------------------------------------------------------------- limpeza

#[tauri::command]
fn clean_categories() -> Vec<cleaner::Category> {
    cleaner::categories()
}

#[tauri::command]
async fn clean_scan(ids: Vec<String>) -> Result<Vec<cleaner::Report>, String> {
    blocking(move || Ok(cleaner::scan(&ids))).await
}

#[tauri::command]
async fn clean_run(ids: Vec<String>) -> Result<Vec<cleaner::Report>, String> {
    blocking(move || {
        let r = cleaner::clean(&ids);
        let mb: u64 = r.iter().map(|x| x.bytes).sum::<u64>() / (1024 * 1024);
        settings::update(|s| s.last_clean = activity::now()).ok();
        activity::log("clean", format!("Limpeza: {mb} MB liberados"));
        Ok(r)
    })
    .await
}

// --------------------------------------------------------------- manutenção

#[tauri::command]
async fn maint_run(app: tauri::AppHandle, task: String) -> Result<String, String> {
    let name = task.clone();
    blocking(move || {
        let t: &'static str = if task == "sfc" { "sfc" } else { "dism" };
        logged(maint::run_tool(&app, t), "system", || format!("Verificação do Windows concluída: {}", name.to_uppercase()))
    })
    .await
}

#[tauri::command]
async fn disk_scan(app: tauri::AppHandle, root: String) -> Result<Vec<maint::DirSize>, String> {
    blocking(move || maint::scan(&app, &root)).await
}

#[tauri::command]
fn disk_scan_cancel() {
    maint::cancel_scan();
}

#[tauri::command]
fn disk_roots() -> Vec<(String, String)> {
    maint::disk_roots()
}

#[tauri::command]
async fn programs() -> Result<Vec<maint::Program>, String> {
    blocking(|| Ok(maint::programs())).await
}

#[tauri::command]
async fn uninstall_program(id: String) -> Result<String, String> {
    blocking(move || {
        let r = maint::uninstall(&id);
        if let Ok(name) = &r {
            activity::log("app", format!("Desinstalador aberto: {name}"));
        }
        r
    })
    .await
}

#[tauri::command]
async fn apps_list() -> Result<Vec<apps::App>, String> {
    blocking(apps::installed).await
}

#[tauri::command]
async fn apps_remove(package: String) -> Result<(), String> {
    blocking(move || logged(apps::remove(&package), "app", || format!("Aplicativo removido: {package}"))).await
}

// --------------------------------------------------- exportar e importar

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    version: u32,
    settings: settings::Settings,
    tweaks: Vec<String>,
}

#[derive(Serialize)]
struct ImportReport {
    applied: u32,
    failed: Vec<String>,
}

#[tauri::command]
async fn config_export() -> Result<String, String> {
    blocking(|| {
        let tweaks = tweaks::list()?.into_iter().filter(|t| t.applied()).map(|t| t.id().to_string()).collect();
        let cfg = ConfigFile { version: 1, settings: settings::get(), tweaks };
        let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        let home = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
        let dir = std::path::PathBuf::from(home).join("Documents").join("Nexus Backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(format!("nexus-config-{}.json", activity::now())), &text).map_err(|e| e.to_string())?;
        activity::log("system", "Configuração exportada");
        Ok(text)
    })
    .await
}

#[tauri::command]
async fn config_import(app: tauri::AppHandle, text: String) -> Result<ImportReport, String> {
    blocking(move || {
        let cfg: ConfigFile = serde_json::from_str(&text).map_err(|e| format!("arquivo de configuração inválido: {e}"))?;
        let keep_wizard = settings::get().wizard_done;
        let s = settings::replace(settings::Settings { wizard_done: keep_wizard, ..cfg.settings })?;
        let _ = app.emit("settings", &s);
        let (mut applied, mut failed) = (0, Vec::new());
        for id in cfg.tweaks {
            if tweaks::tweak_applied(&id) {
                continue;
            }
            match tweaks::apply(&id) {
                Ok(()) => applied += 1,
                Err(e) => failed.push(format!("{}: {e}", tweaks::name_of(&id))),
            }
        }
        activity::log("system", format!("Configuração importada ({applied} ajuste(s) aplicados)"));
        Ok(ImportReport { applied, failed })
    })
    .await
}

// ------------------------------------------------------------------- início

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(shell::hotkey_plugin())
        .on_window_event(|window, event| {
            // Com "minimizar para a bandeja", fechar a janela só a esconde.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && settings::get().minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            use tauri::Manager;
            let handle = app.handle().clone();
            // Ícone da janela e da barra de tarefas definido em tempo de execução. O include_bytes!
            // faz o Cargo recompilar quando o arquivo muda.
            if let (Some(win), Ok(icon)) = (
                app.get_webview_window("main"),
                tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png")),
            ) {
                let _ = win.set_icon(icon);
                if std::env::args().any(|a| a == "--minimized") {
                    let _ = win.hide();
                }
            }
            let _ = shell::setup_tray(&handle);
            shell::register_hotkeys(&handle);
            gamemode::start(handle.clone());
            shell::start_scheduler(handle.clone());
            tauri::async_runtime::spawn(async { games::ensure(false).await });
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                if settings::get().check_updates {
                    if let Ok(info) = shell::check_update().await {
                        if info.newer {
                            shell::notify(&h, "Nexus Optimizer", &format!("Nova versão disponível: {}", info.latest));
                            let _ = h.emit("update", &info);
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            relaunch_admin, is_admin, app_version, autostart_get, autostart_set, settings_get, settings_set,
            activity_list, activity_clear, open_target, open_report, open_reports_folder, check_update,
            list_tweaks, apply_tweak, revert_tweak,
            run_benchmark, bench_history, bench_delete, bench_clear, ab_verdict, bench_report,
            catalog_status, running_games, search_games, gamemode_get, gamemode_set, free_memory, cpu_count,
            set_affinity_by_name,
            server_list, ping_servers, tcp_ping, dns_benchmark, route_trace, gpu_stats, disk_health, drivers,
            msi_list, msi_set, speed_test, ping_test, get_dns, set_dns, restore_point, set_priority,
            usage, processes, kill_process, startup_list, startup_set, system_info,
            clean_categories, clean_scan, clean_run,
            maint_run, disk_scan, disk_scan_cancel, disk_roots, programs, uninstall_program, apps_list, apps_remove,
            config_export, config_import
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
