use serde::Serialize;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const APPROVED_RUN32: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run32";

struct Loc {
    hklm: bool,
    run: &'static str,
    approved: &'static str,
    scope: &'static str,
}

const LOCS: &[Loc] = &[
    Loc { hklm: false, run: RUN, approved: APPROVED_RUN, scope: "Usuário" },
    Loc { hklm: true, run: RUN, approved: APPROVED_RUN, scope: "Sistema" },
    Loc { hklm: true, run: RUN32, approved: APPROVED_RUN32, scope: "Sistema (32 bits)" },
];

#[derive(Serialize)]
pub struct StartupItem {
    pub loc: usize,
    pub name: String,
    pub command: String,
    pub scope: &'static str,
    pub enabled: bool,
    pub admin: bool,
}

fn root(hklm: bool) -> RegKey {
    RegKey::predef(if hklm { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

/// Mesmo mecanismo do Gerenciador de Tarefas: o primeiro byte do valor em StartupApproved
/// indica o estado (par = ativo, ímpar = desativado). Não apagamos nada da chave Run.
fn is_enabled(loc: &Loc, name: &str) -> bool {
    root(loc.hklm)
        .open_subkey(loc.approved)
        .and_then(|k| k.get_raw_value(name))
        .map(|v| v.bytes.first().map(|b| b % 2 == 0).unwrap_or(true))
        .unwrap_or(true)
}

pub fn list() -> Vec<StartupItem> {
    let mut out = Vec::new();
    for (i, loc) in LOCS.iter().enumerate() {
        let Ok(key) = root(loc.hklm).open_subkey(loc.run) else { continue };
        for (name, value) in key.enum_values().flatten() {
            let command = value.to_string();
            out.push(StartupItem {
                loc: i,
                enabled: is_enabled(loc, &name),
                name,
                command,
                scope: loc.scope,
                admin: loc.hklm,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn filetime_now() -> u64 {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (unix + 11_644_473_600) * 10_000_000
}

pub fn state_bytes(enabled: bool) -> Vec<u8> {
    let mut b = vec![0u8; 12];
    if enabled {
        b[0] = 0x02;
    } else {
        b[0] = 0x03;
        b[4..12].copy_from_slice(&filetime_now().to_le_bytes());
    }
    b
}

pub fn set_enabled(loc: usize, name: &str, enabled: bool) -> Result<(), String> {
    let l = LOCS.get(loc).ok_or("local de inicialização inválido")?;
    if l.hklm && !crate::tweaks::is_admin() {
        return Err("Este item é do sistema: abra o app como administrador.".into());
    }
    // Só mexe em itens que realmente existem na chave Run.
    let exists = root(l.hklm).open_subkey(l.run).and_then(|k| k.get_raw_value(name)).is_ok();
    if !exists {
        return Err("item de inicialização não encontrado".into());
    }
    let (key, _) = root(l.hklm).create_subkey(l.approved).map_err(|e| e.to_string())?;
    let value = RegValue { bytes: state_bytes(enabled), vtype: REG_BINARY };
    key.set_raw_value(name, &value).map_err(|e| e.to_string())
}

const SELF_NAME: &str = "NexusOptimizer";

/// O Nexus está registrado para iniciar com o Windows (e não foi desativado no Gerenciador de Tarefas)?
pub fn autostart_enabled() -> bool {
    list().iter().any(|i| i.loc == 0 && i.name == SELF_NAME && i.enabled)
}

/// Liga/desliga o início com o Windows para o usuário atual (sem administrador). Usa a chave Run
/// e, ao ligar, garante que o item não esteja marcado como desativado em StartupApproved.
pub fn set_autostart(on: bool) -> Result<(), String> {
    let run = root(false).open_subkey_with_flags(LOCS[0].run, KEY_SET_VALUE | KEY_READ).map_err(|e| e.to_string())?;
    if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        run.set_value(SELF_NAME, &format!("\"{}\" --minimized", exe.display())).map_err(|e| e.to_string())?;
        set_enabled(0, SELF_NAME, true)
    } else {
        match run.delete_value(SELF_NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_encoding() {
        let on = state_bytes(true);
        let off = state_bytes(false);
        assert_eq!((on.len(), off.len()), (12, 12));
        assert_eq!(on[0] % 2, 0);
        assert_eq!(off[0] % 2, 1);
        assert!(off[4..].iter().any(|b| *b != 0), "timestamp preenchido");
    }

    // Liga e desliga o início com o Windows de verdade (HKCU\Run); pula se o usuário já o ativou.
    #[test]
    fn autostart_roundtrip() {
        if autostart_enabled() {
            eprintln!("autostart já ativo no sistema; teste pulado para não alterar seu estado");
            return;
        }
        set_autostart(true).unwrap();
        assert!(autostart_enabled());
        set_autostart(false).unwrap();
        assert!(!autostart_enabled());
        set_autostart(false).unwrap(); // desligar de novo não pode falhar
    }

    #[test]
    fn list_reads_without_error_and_rejects_bad_input() {
        let _ = list();
        assert!(set_enabled(99, "x", false).is_err());
        assert!(set_enabled(0, "item-que-nao-existe-nexus", false).is_err());
    }
}
