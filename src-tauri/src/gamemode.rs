use serde::Serialize;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA};

use crate::settings::{self, Profile};

const POWER_TWEAK: &str = "power-ultimate";
const POLL: Duration = Duration::from_secs(3);

static STATUS: Mutex<Status> = Mutex::new(Status { active: false, game: None });

#[derive(Serialize, Clone, Default)]
pub struct Status {
    pub active: bool,
    pub game: Option<String>,
}

pub fn is_enabled() -> bool {
    settings::get().auto_game_mode
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    settings::update(|s| s.auto_game_mode = on).map(|_| ())
}

pub fn status() -> Status {
    STATUS.lock().unwrap().clone()
}

/// Esvazia o working set de cada processo (exceto jogos detectados). O efeito é temporário: o Windows
/// devolve as páginas quando o programa precisa delas. Devolve (usado antes, usado depois) em bytes.
pub fn free_memory() -> (u64, u64) {
    let cat = crate::games::current();
    let protected: HashSet<u32> = cat
        .as_ref()
        .map(|c| {
            crate::sys::with_processes(|s| crate::games::detect(s, c))
                .into_iter()
                .flat_map(|r| r.pids)
                .collect()
        })
        .unwrap_or_default();
    let before = crate::sys::usage().ram_used;
    let pids: Vec<u32> = crate::sys::with_processes(|s| s.processes().keys().map(|p| p.as_u32()).collect());
    for pid in pids {
        if pid == 0 || pid == 4 || protected.contains(&pid) {
            continue;
        }
        // SAFETY: handle aberto e fechado nesta iteração; falhas (sem permissão) são ignoradas.
        unsafe {
            let h = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, 0, pid);
            if !h.is_null() {
                K32EmptyWorkingSet(h);
                CloseHandle(h);
            }
        }
    }
    std::thread::sleep(Duration::from_millis(400));
    (before, crate::sys::usage().ram_used)
}

/// Perfil do jogo: o específico (pelo executável) ou o padrão.
pub fn profile_for(exe: &str) -> Profile {
    let s = settings::get();
    s.profiles
        .iter()
        .find(|p| p.exe.eq_ignore_ascii_case(exe))
        .map(|p| p.profile.clone())
        .unwrap_or(s.default_profile)
}

struct Active {
    exe: String,
    name: String,
    power_by_us: bool,
    tweaks_by_us: Vec<String>,
    handled: HashSet<u32>,
    profile: Profile,
}

fn publish(app: &AppHandle, st: Status) {
    *STATUS.lock().unwrap() = st.clone();
    let _ = app.emit("gamemode", st);
}

fn apply_to_pids(a: &mut Active, pids: &[u32]) {
    for &p in pids {
        if a.handled.contains(&p) {
            continue;
        }
        let mut ok = crate::hw::set_priority(p, &a.profile.priority);
        if let Some(mask) = a.profile.affinity {
            ok |= crate::hw::set_affinity(p, mask);
        }
        if ok {
            a.handled.insert(p);
        }
    }
}

fn activate(app: &AppHandle, name: String, exe: String, pids: Vec<u32>) -> Active {
    let profile = profile_for(&exe);
    let power_by_us = !crate::tweaks::tweak_applied(POWER_TWEAK) && crate::tweaks::apply(POWER_TWEAK).is_ok();
    let mut tweaks_by_us = Vec::new();
    for id in &profile.tweaks {
        if !crate::tweaks::tweak_applied(id) && crate::tweaks::apply(id).is_ok() {
            tweaks_by_us.push(id.clone());
        }
    }
    let mut closed = Vec::new();
    for app_name in &profile.close_apps {
        if !app_name.eq_ignore_ascii_case(&exe) && crate::sys::kill(app_name).is_ok() {
            closed.push(app_name.clone());
        }
    }
    let mut a = Active { exe, name: name.clone(), power_by_us, tweaks_by_us, handled: HashSet::new(), profile };
    apply_to_pids(&mut a, &pids);
    crate::activity::log(
        "profile",
        format!(
            "Modo Jogo ativado em {name} (prioridade {}, {} ajuste(s), {} app(s) fechados)",
            a.profile.priority,
            a.tweaks_by_us.len(),
            closed.len()
        ),
    );
    crate::shell::notify(app, "Nexus Optimizer", &format!("Modo Jogo ativo: {name}"));
    publish(app, Status { active: true, game: Some(name) });
    a
}

fn deactivate(app: &AppHandle, a: Active) {
    for id in a.tweaks_by_us.iter().rev() {
        let _ = crate::tweaks::revert(id);
    }
    if a.power_by_us {
        let _ = crate::tweaks::revert(POWER_TWEAK);
    }
    crate::activity::log("profile", format!("Modo Jogo encerrado ({}): ajustes do perfil revertidos", a.name));
    publish(app, Status::default());
}

/// Vigia em segundo plano: quando um jogo do catálogo abre, aplica o perfil dele; quando fecha, desfaz
/// tudo que o perfil mudou. Só age com o Modo Jogo automático ligado.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut active: Option<Active> = None;
        loop {
            std::thread::sleep(POLL);
            let found = if is_enabled() {
                crate::games::current().and_then(|cat| {
                    crate::sys::with_processes(|s| crate::games::detect(s, &cat))
                        .into_iter()
                        .next()
                        .map(|r| (cat.games[r.game].name.clone(), r.exe, r.pids))
                })
            } else {
                None
            };
            match (&mut active, found) {
                (None, Some((name, exe, pids))) => active = Some(activate(&app, name, exe, pids)),
                (Some(a), Some((_, exe, pids))) if exe == a.exe => apply_to_pids(a, &pids),
                (Some(_), _) => {
                    if let Some(a) = active.take() {
                        deactivate(&app, a);
                    }
                }
                (None, None) => {}
            }
        }
    });
}
