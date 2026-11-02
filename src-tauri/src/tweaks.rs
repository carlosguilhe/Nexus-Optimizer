use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ULTIMATE_SCHEME: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
const HIGH_SCHEME: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
const SERVICES_PREFIX: &str = r"SYSTEM\CurrentControlSet\Services\";

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
enum Hive {
    #[default]
    Cu,
    Lm,
}

const CU: Hive = Hive::Cu;
const LM: Hive = Hive::Lm;

#[derive(Clone, Copy)]
enum Val {
    Dword(u32),
    Str(&'static str),
}

struct RegEntry {
    hive: Hive,
    path: &'static str,
    name: &'static str,
    value: Val,
}

const fn dw(hive: Hive, path: &'static str, name: &'static str, v: u32) -> RegEntry {
    RegEntry { hive, path, name, value: Val::Dword(v) }
}

const fn sz(hive: Hive, path: &'static str, name: &'static str, v: &'static str) -> RegEntry {
    RegEntry { hive, path, name, value: Val::Str(v) }
}

/// Serviço: desabilita via Start=4 (efeito após reiniciar). Reversível pelo backup.
macro_rules! svc {
    ($name:literal) => {
        dw(LM, concat!(r"SYSTEM\CurrentControlSet\Services\", $name), "Start", 4)
    };
}

enum Kind {
    Registry(&'static [RegEntry]),
    PowerUltimate,
    PowerNexus,
}

struct Def {
    id: &'static str,
    category: &'static str,
    name: &'static str,
    description: &'static str,
    risk: &'static str,
    evidence: &'static str,
    reboot: bool,
    kind: Kind,
}

const NVT: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const GCS: &str = r"System\GameConfigStore";

// Regra: todo ajuste tem backup antes de aplicar e reversão. Evidência é descrita com honestidade;
// o ganho real só vale depois de medir no benchmark.
const DEFS: &[Def] = &[
    // ---------- Jogos ----------
    Def {
        id: "game-mode",
        category: "Jogos",
        name: "Modo de Jogo do Windows",
        description: "Ativa o Game Mode, que reduz tarefas em segundo plano durante jogos.",
        risk: "seguro",
        evidence: "Ganho pequeno; meça com o benchmark antes de confiar.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(CU, r"Software\Microsoft\GameBar", "AllowAutoGameMode", 1),
            dw(CU, r"Software\Microsoft\GameBar", "AutoGameModeEnabled", 1),
        ]),
    },
    Def {
        id: "gamedvr-off",
        category: "Jogos",
        name: "Desativar captura em segundo plano (Game DVR)",
        description: "Impede a gravação em segundo plano da Xbox Game Bar.",
        risk: "seguro",
        evidence: "Pode reduzir overhead em GPUs fracas; ganho tipicamente pequeno.",
        reboot: false,
        kind: Kind::Registry(&[dw(CU, GCS, "GameDVR_Enabled", 0)]),
    },
    Def {
        id: "gamebar-popups-off",
        category: "Jogos",
        name: "Desativar pop-ups da Game Bar",
        description: "Remove o painel de inicialização e dicas da Xbox Game Bar durante os jogos.",
        risk: "seguro",
        evidence: "Evita interrupções; não altera FPS.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(CU, r"Software\Microsoft\GameBar", "ShowStartupPanel", 0),
            dw(CU, r"Software\Microsoft\GameBar", "UseNexusForGameBarEnabled", 0),
        ]),
    },
    Def {
        id: "fso-off",
        category: "Jogos",
        name: "Desativar otimizações de tela cheia (global)",
        description: "Faz os jogos usarem tela cheia exclusiva tradicional em vez do modo otimizado do Windows.",
        risk: "moderado",
        evidence: "Pode reduzir input lag e stutter em alguns jogos; em outros não muda. Meça.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(CU, GCS, "GameDVR_FSEBehaviorMode", 2),
            dw(CU, GCS, "GameDVR_HonorUserFSEBehaviorMode", 1),
            dw(CU, GCS, "GameDVR_DXGIHonorFSEWindowsCompatible", 1),
        ]),
    },
    Def {
        id: "windowed-optimizations",
        category: "Jogos",
        name: "Otimizações para jogos em janela",
        description: "Ativa o upgrade de swap effect do DirectX (menor latência em jogos em janela/sem borda).",
        risk: "seguro",
        evidence: "Só afeta jogos DX10/11 em janela; ganho de latência, não de FPS.",
        reboot: false,
        kind: Kind::Registry(&[sz(
            CU,
            r"Software\Microsoft\DirectX\UserGpuPreferences",
            "DirectXUserGlobalSettings",
            "SwapEffectUpgradeEnable=1;",
        )]),
    },
    Def {
        id: "mmcss-games",
        category: "Jogos",
        name: "Prioridade de jogos no agendador (MMCSS)",
        description: "Eleva a prioridade e a categoria de agendamento da tarefa 'Games' do Windows.",
        risk: "moderado",
        evidence: "Ganho pequeno e incerto; comum em guias, pouco medido. Meça.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(LM, concat!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile", r"\Tasks\Games"), "Priority", 6),
            sz(LM, concat!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile", r"\Tasks\Games"), "Scheduling Category", "High"),
            sz(LM, concat!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile", r"\Tasks\Games"), "SFIO Priority", "High"),
        ]),
    },
    Def {
        id: "hags-on",
        category: "Jogos",
        name: "Agendamento de GPU acelerado por hardware (HAGS)",
        description: "Deixa a GPU gerenciar a própria memória de vídeo. Exige GPU/driver compatível.",
        risk: "moderado",
        evidence: "Pode ajudar, não mudar ou piorar conforme jogo e driver. Meça antes e depois.",
        reboot: true,
        kind: Kind::Registry(&[dw(LM, r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers", "HwSchMode", 2)]),
    },
    // ---------- Energia ----------
    Def {
        id: "power-ultimate",
        category: "Energia",
        name: "Plano de energia Desempenho Máximo",
        description: "Cria e ativa o plano Ultimate Performance. O plano anterior é restaurado ao reverter.",
        risk: "seguro",
        evidence: "Maior efeito em notebooks e CPUs com economia agressiva; aumenta consumo/temperatura.",
        reboot: false,
        kind: Kind::PowerUltimate,
    },
    Def {
        id: "power-nexus",
        category: "Energia",
        name: "Plano de energia Nexus Performance",
        description: "Cria um plano próprio: CPU sempre no máximo com boost agressivo, PCIe e disco sem economia. O plano anterior volta ao reverter.",
        risk: "moderado",
        evidence: "Pode reduzir microtravadas causadas por economia de energia; aumenta consumo e temperatura.",
        reboot: false,
        kind: Kind::PowerNexus,
    },
    Def {
        id: "fast-startup-off",
        category: "Energia",
        name: "Desativar inicialização rápida",
        description: "Faz o desligar ser completo, evitando drivers e estado travados entre sessões.",
        risk: "seguro",
        evidence: "Não aumenta FPS; ajuda a resolver bugs que só somem reiniciando.",
        reboot: true,
        kind: Kind::Registry(&[dw(LM, r"SYSTEM\CurrentControlSet\Control\Session Manager\Power", "HiberbootEnabled", 0)]),
    },
    // ---------- Entrada ----------
    Def {
        id: "mouse-accel-off",
        category: "Entrada",
        name: "Desativar aceleração do mouse",
        description: "Desliga 'Aprimorar precisão do ponteiro'. Movimento 1:1, mira mais consistente.",
        risk: "seguro",
        evidence: "Efeito real e mensurável na consistência da mira (não em FPS).",
        reboot: false,
        kind: Kind::Registry(&[
            sz(CU, r"Control Panel\Mouse", "MouseSpeed", "0"),
            sz(CU, r"Control Panel\Mouse", "MouseThreshold1", "0"),
            sz(CU, r"Control Panel\Mouse", "MouseThreshold2", "0"),
        ]),
    },
    Def {
        id: "timer-resolution",
        category: "Entrada",
        name: "Resolução de timer global",
        description: "Permite que pedidos de timer de alta resolução valham para todo o sistema.",
        risk: "moderado",
        evidence: "Só tem efeito no Windows 11 22H2+ e pode aumentar consumo. Incerto; meça.",
        reboot: true,
        kind: Kind::Registry(&[dw(LM, r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel", "GlobalTimerResolutionRequests", 1)]),
    },
    Def {
        id: "sticky-keys-off",
        category: "Entrada",
        name: "Desativar atalhos de acessibilidade do teclado",
        description: "Impede o pop-up das Teclas de Aderência ao apertar Shift 5 vezes (e similares) em jogos.",
        risk: "seguro",
        evidence: "Evita sair do jogo sem querer; não altera FPS.",
        reboot: false,
        kind: Kind::Registry(&[
            sz(CU, r"Control Panel\Accessibility\StickyKeys", "Flags", "506"),
            sz(CU, r"Control Panel\Accessibility\Keyboard Response", "Flags", "122"),
            sz(CU, r"Control Panel\Accessibility\ToggleKeys", "Flags", "58"),
        ]),
    },
    Def {
        id: "keyboard-fast",
        category: "Entrada",
        name: "Teclado com repetição mais rápida",
        description: "Reduz o atraso e acelera a repetição das teclas ao segurar uma.",
        risk: "seguro",
        evidence: "Mais responsivo ao digitar e navegar; sem efeito em FPS. Pode exigir sair e entrar na conta.",
        reboot: true,
        kind: Kind::Registry(&[
            sz(CU, r"Control Panel\Keyboard", "KeyboardDelay", "0"),
            sz(CU, r"Control Panel\Keyboard", "KeyboardSpeed", "31"),
        ]),
    },
    // ---------- Rede ----------
    Def {
        id: "delivery-opt-off",
        category: "Rede",
        name: "Desativar compartilhamento de atualizações (P2P)",
        description: "Impede o Windows de enviar atualizações para outros PCs usando a sua internet.",
        risk: "seguro",
        evidence: "Evita consumo de upload em segundo plano, que pode causar picos de ping.",
        reboot: false,
        kind: Kind::Registry(&[dw(LM, r"SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization", "DODownloadMode", 0)]),
    },
    Def {
        id: "network-throttling-off",
        category: "Rede",
        name: "Desativar limitação de rede do multimídia",
        description: "Remove o limite de pacotes que o Windows aplica quando há multimídia ativa.",
        risk: "moderado",
        evidence: "Pouco efeito em jogos; não reduz o ping, que depende da rota até o servidor.",
        reboot: false,
        kind: Kind::Registry(&[dw(LM, NVT, "NetworkThrottlingIndex", 0xFFFF_FFFF)]),
    },
    // ---------- Visual ----------
    Def {
        id: "transparency-off",
        category: "Visual",
        name: "Desativar efeitos de transparência",
        description: "Remove a transparência do Windows. Útil em vídeo integrado ou PC fraco.",
        risk: "seguro",
        evidence: "Só faz diferença em hardware fraco.",
        reboot: false,
        kind: Kind::Registry(&[dw(
            CU,
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "EnableTransparency",
            0,
        )]),
    },
    Def {
        id: "visual-perf",
        category: "Visual",
        name: "Interface mais rápida",
        description: "Menus sem atraso e animações da barra de tarefas desativadas.",
        risk: "seguro",
        evidence: "Deixa a interface mais ágil; não altera FPS em jogos.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects", "VisualFXSetting", 2),
            sz(CU, r"Control Panel\Desktop", "MenuShowDelay", "0"),
            dw(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced", "TaskbarAnimations", 0),
        ]),
    },
    // ---------- Limpeza e privacidade ----------
    Def {
        id: "tips-ads-off",
        category: "Privacidade",
        name: "Remover sugestões e apps patrocinados",
        description: "Desativa anúncios no Iniciar, dicas e reinstalação automática de apps patrocinados.",
        risk: "seguro",
        evidence: "Evita que o 'bloatware' volte; sem ganho de FPS.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(CU, CDM, "SystemPaneSuggestionsEnabled", 0),
            dw(CU, CDM, "SilentInstalledAppsEnabled", 0),
            dw(CU, CDM, "SoftLandingEnabled", 0),
            dw(CU, CDM, "OemPreInstalledAppsEnabled", 0),
            dw(CU, CDM, "PreInstalledAppsEnabled", 0),
            dw(CU, CDM, "SubscribedContent-338388Enabled", 0),
            dw(CU, CDM, "SubscribedContent-338389Enabled", 0),
            dw(CU, CDM, "SubscribedContent-353694Enabled", 0),
            dw(CU, CDM, "SubscribedContent-353696Enabled", 0),
        ]),
    },
    Def {
        id: "ad-id-off",
        category: "Privacidade",
        name: "Desativar ID de publicidade",
        description: "Impede que apps usem seu identificador para anúncios personalizados.",
        risk: "seguro",
        evidence: "Privacidade; sem efeito em desempenho.",
        reboot: false,
        kind: Kind::Registry(&[dw(CU, r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", "Enabled", 0)]),
    },
    Def {
        id: "background-apps-off",
        category: "Privacidade",
        name: "Bloquear apps em segundo plano",
        description: "Impede apps da Microsoft Store de rodar em segundo plano.",
        risk: "moderado",
        evidence: "Libera um pouco de RAM/CPU; alguns apps deixam de dar notificações.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(CU, r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications", "GlobalUserDisabled", 1),
            dw(CU, r"Software\Microsoft\Windows\CurrentVersion\Search", "BackgroundAppGlobalToggle", 0),
        ]),
    },
    Def {
        id: "widgets-off",
        category: "Privacidade",
        name: "Desativar Widgets",
        description: "Desliga o painel de Widgets/Notícias, que mantém processos rodando.",
        risk: "seguro",
        evidence: "Libera algumas centenas de MB de RAM em PCs com Widgets ativo.",
        reboot: true,
        kind: Kind::Registry(&[dw(LM, r"SOFTWARE\Policies\Microsoft\Dsh", "AllowNewsAndInterests", 0)]),
    },
    Def {
        id: "telemetry-min",
        category: "Privacidade",
        name: "Reduzir telemetria do Windows",
        description: "Configura a coleta de diagnóstico para o mínimo permitido na sua edição.",
        risk: "seguro",
        evidence: "Privacidade; efeito em desempenho desprezível.",
        reboot: false,
        kind: Kind::Registry(&[dw(LM, r"SOFTWARE\Policies\Microsoft\Windows\DataCollection", "AllowTelemetry", 0)]),
    },
    Def {
        id: "activity-history-off",
        category: "Privacidade",
        name: "Desativar histórico de atividades",
        description: "Para de registrar e enviar o histórico de atividades (Timeline).",
        risk: "seguro",
        evidence: "Privacidade; sem ganho de FPS.",
        reboot: false,
        kind: Kind::Registry(&[
            dw(LM, r"SOFTWARE\Policies\Microsoft\Windows\System", "EnableActivityFeed", 0),
            dw(LM, r"SOFTWARE\Policies\Microsoft\Windows\System", "PublishUserActivities", 0),
            dw(LM, r"SOFTWARE\Policies\Microsoft\Windows\System", "UploadUserActivities", 0),
        ]),
    },
    // ---------- Serviços ----------
    Def {
        id: "svc-telemetry",
        category: "Serviços",
        name: "Desativar serviços de telemetria",
        description: "Desabilita DiagTrack e dmwappushservice (coleta e envio de diagnóstico).",
        risk: "moderado",
        evidence: "Pouca RAM/CPU liberada; reduz tráfego em segundo plano. Efeito após reiniciar.",
        reboot: true,
        kind: Kind::Registry(&[svc!("DiagTrack"), svc!("dmwappushservice")]),
    },
    Def {
        id: "svc-wer",
        category: "Serviços",
        name: "Desativar relatório de erros do Windows",
        description: "Desabilita o WerSvc, que coleta e envia relatórios de falhas.",
        risk: "seguro",
        evidence: "Evita pausas após travamentos; ganho pequeno.",
        reboot: true,
        kind: Kind::Registry(&[svc!("WerSvc")]),
    },
    Def {
        id: "svc-useless",
        category: "Serviços",
        name: "Desativar serviços raramente úteis",
        description: "Desabilita Mapas baixados (MapsBroker), modo demonstração (RetailDemo) e compartilhamento de mídia (WMPNetworkSvc).",
        risk: "seguro",
        evidence: "Ganho pequeno; só desativa serviços que existem no seu Windows.",
        reboot: true,
        kind: Kind::Registry(&[svc!("MapsBroker"), svc!("RetailDemo"), svc!("WMPNetworkSvc")]),
    },
    Def {
        id: "svc-sysmain",
        category: "Serviços",
        name: "Desativar SysMain (Superfetch)",
        description: "Desabilita o pré-carregamento de apps em memória.",
        risk: "moderado",
        evidence: "Em SSD é inofensivo e libera RAM/disco; em HD costuma piorar. Use só com SSD.",
        reboot: true,
        kind: Kind::Registry(&[svc!("SysMain")]),
    },
    Def {
        id: "svc-search",
        category: "Serviços",
        name: "Desativar indexação de busca (WSearch)",
        description: "Para o indexador do Windows Search, que usa disco e CPU em segundo plano.",
        risk: "moderado",
        evidence: "Libera disco/CPU, mas a busca no Iniciar e no Explorer fica mais lenta.",
        reboot: true,
        kind: Kind::Registry(&[svc!("WSearch")]),
    },
    // ---------- Sistema ----------
    Def {
        id: "startup-delay-off",
        category: "Sistema",
        name: "Sem atraso na abertura de programas de inicialização",
        description: "Remove o atraso artificial que o Windows aplica aos apps que iniciam com o sistema.",
        risk: "seguro",
        evidence: "Área de trabalho fica pronta mais cedo após o login.",
        reboot: true,
        kind: Kind::Registry(&[dw(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize", "StartupDelayInMSec", 0)]),
    },
    Def {
        id: "fast-shutdown",
        category: "Sistema",
        name: "Desligar mais rápido",
        description: "Reduz para 2 s o tempo de espera para encerrar serviços e apps travados no desligamento.",
        risk: "moderado",
        evidence: "Desliga mais rápido; um app lento para salvar pode ser encerrado antes de terminar.",
        reboot: true,
        kind: Kind::Registry(&[
            sz(LM, r"SYSTEM\CurrentControlSet\Control", "WaitToKillServiceTimeout", "2000"),
            sz(CU, r"Control Panel\Desktop", "WaitToKillAppTimeout", "2000"),
        ]),
    },
    Def {
        id: "edge-bg-off",
        category: "Sistema",
        name: "Edge sem processos em segundo plano",
        description: "Desativa o Startup Boost e a execução em segundo plano do Microsoft Edge.",
        risk: "seguro",
        evidence: "Libera RAM e CPU de processos do Edge que ficam abertos sem janela.",
        reboot: true,
        kind: Kind::Registry(&[
            dw(LM, r"SOFTWARE\Policies\Microsoft\Edge", "StartupBoostEnabled", 0),
            dw(LM, r"SOFTWARE\Policies\Microsoft\Edge", "BackgroundModeEnabled", 0),
        ]),
    },
    Def {
        id: "search-web-off",
        category: "Sistema",
        name: "Busca do Iniciar sem resultados da web",
        description: "A busca do Windows passa a mostrar só itens do PC, sem sugestões da internet.",
        risk: "seguro",
        evidence: "Busca mais rápida e limpa; privacidade.",
        reboot: true,
        kind: Kind::Registry(&[dw(CU, r"Software\Policies\Microsoft\Windows\Explorer", "DisableSearchBoxSuggestions", 1)]),
    },
    Def {
        id: "file-ext-visible",
        category: "Sistema",
        name: "Mostrar extensões de arquivos",
        description: "Exibe .exe, .zip etc. nos nomes. Ajuda a identificar arquivos disfarçados.",
        risk: "seguro",
        evidence: "Segurança: evita executar 'foto.jpg.exe' achando que é imagem.",
        reboot: false,
        kind: Kind::Registry(&[dw(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced", "HideFileExt", 0)]),
    },
    Def {
        id: "toasts-off",
        category: "Sistema",
        name: "Silenciar notificações do Windows",
        description: "Desativa as notificações em pop-up, que atrapalham jogos em tela cheia.",
        risk: "moderado",
        evidence: "Evita interrupções; você deixa de ver avisos de apps.",
        reboot: false,
        kind: Kind::Registry(&[dw(CU, r"Software\Microsoft\Windows\CurrentVersion\PushNotifications", "ToastEnabled", 0)]),
    },
    // ---------- Avançado ----------
    Def {
        id: "hvci-off",
        category: "Avançado",
        name: "Desativar Integridade de Memória (HVCI)",
        description: "Desliga a proteção por virtualização que verifica drivers do kernel.",
        risk: "avançado",
        evidence: "Pode dar alguns % de FPS em certos PCs, mas reduz a proteção contra drivers maliciosos. Só se medir ganho real.",
        reboot: true,
        kind: Kind::Registry(&[dw(
            LM,
            r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
            "Enabled",
            0,
        )]),
    },
];

#[derive(Serialize, Deserialize, Clone)]
enum SavedVal {
    Missing,
    Dword(u32),
    Str(String),
}

#[derive(Serialize, Deserialize, Clone)]
struct SavedEntry {
    #[serde(default)]
    hive: Hive,
    path: String,
    name: String,
    orig: SavedVal,
}

#[derive(Serialize, Deserialize, Clone)]
enum Saved {
    Reg(Vec<SavedEntry>),
    Power { original: String, created: String },
}

type Backup = HashMap<String, Saved>;

#[derive(Serialize)]
pub struct TweakInfo {
    id: &'static str,
    category: &'static str,
    name: &'static str,
    description: &'static str,
    risk: &'static str,
    evidence: &'static str,
    applied: bool,
    admin: bool,
    reboot: bool,
}

/// Pasta de dados do app (backup, catálogo de jogos, configurações).
pub fn data_dir() -> Result<PathBuf, String> {
    let dir = match std::env::var("NEXUS_DATA_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => PathBuf::from(std::env::var("APPDATA").map_err(|e| e.to_string())?)
            .join("NexusOptimizer"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn backup_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("backup.json"))
}

/// Se o ajuste está ativo agora (usado pelo Modo Jogo automático).
pub fn tweak_applied(id: &str) -> bool {
    match (find(id), load_backup()) {
        (Ok(def), Ok(backup)) => is_applied(def, &backup),
        _ => false,
    }
}

fn load_backup() -> Result<Backup, String> {
    let p = backup_path()?;
    if !p.exists() {
        return Ok(Backup::new());
    }
    let s = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| format!("backup corrompido: {e}"))
}

fn save_backup(b: &Backup) -> Result<(), String> {
    let s = serde_json::to_string_pretty(b).map_err(|e| e.to_string())?;
    std::fs::write(backup_path()?, s).map_err(|e| e.to_string())
}

fn root(h: Hive) -> RegKey {
    RegKey::predef(match h {
        Hive::Cu => HKEY_CURRENT_USER,
        Hive::Lm => HKEY_LOCAL_MACHINE,
    })
}

/// true se o processo atual pode escrever em HKLM (executando como administrador).
pub fn is_admin() -> bool {
    root(Hive::Lm).open_subkey_with_flags("SOFTWARE", KEY_WRITE).is_ok()
}

fn read_saved(hive: Hive, path: &str, name: &str) -> SavedVal {
    let Ok(key) = root(hive).open_subkey(path) else {
        return SavedVal::Missing;
    };
    if let Ok(v) = key.get_value::<u32, _>(name) {
        return SavedVal::Dword(v);
    }
    if let Ok(v) = key.get_value::<String, _>(name) {
        return SavedVal::Str(v);
    }
    SavedVal::Missing
}

/// Serviços que não existem nesta edição do Windows são ignorados (nunca criamos chaves de serviço).
fn skipped(e: &RegEntry) -> bool {
    e.path.starts_with(SERVICES_PREFIX) && root(e.hive).open_subkey(e.path).is_err()
}

fn entry_matches(e: &RegEntry) -> bool {
    match (read_saved(e.hive, e.path, e.name), e.value) {
        (SavedVal::Dword(a), Val::Dword(b)) => a == b,
        (SavedVal::Str(a), Val::Str(b)) => a == b,
        _ => false,
    }
}

fn powercfg(args: &[&str]) -> Result<String, String> {
    let out = Command::new("powercfg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn parse_guid(s: &str) -> Option<String> {
    s.split_whitespace()
        .find(|t| t.len() == 36 && t.matches('-').count() == 4)
        .map(|t| t.to_lowercase())
}

fn find(id: &str) -> Result<&'static Def, String> {
    DEFS.iter().find(|d| d.id == id).ok_or_else(|| format!("ajuste desconhecido: {id}"))
}

fn needs_admin(def: &Def) -> bool {
    matches!(&def.kind, Kind::Registry(es) if es.iter().any(|e| e.hive == Hive::Lm))
}

fn is_applied(def: &Def, backup: &Backup) -> bool {
    match &def.kind {
        Kind::Registry(entries) => {
            let active: Vec<&RegEntry> = entries.iter().filter(|e| !skipped(e)).collect();
            !active.is_empty() && active.into_iter().all(entry_matches)
        }
        Kind::PowerUltimate | Kind::PowerNexus => match backup.get(def.id) {
            Some(Saved::Power { created, .. }) => powercfg(&["/getactivescheme"])
                .ok()
                .and_then(|o| parse_guid(&o))
                .is_some_and(|g| &g == created),
            _ => false,
        },
    }
}

impl TweakInfo {
    pub fn id(&self) -> &str {
        self.id
    }
    pub fn applied(&self) -> bool {
        self.applied
    }
}

/// Nome legível de um ajuste (para o registro de atividades).
pub fn name_of(id: &str) -> String {
    find(id).map(|d| d.name.to_string()).unwrap_or_else(|_| id.to_string())
}

/// Risco declarado de um ajuste (seguro | moderado | avançado).
pub fn risk_of(id: &str) -> Option<&'static str> {
    find(id).ok().map(|d| d.risk)
}

pub fn list() -> Result<Vec<TweakInfo>, String> {
    let backup = load_backup()?;
    Ok(DEFS
        .iter()
        .map(|d| TweakInfo {
            id: d.id,
            category: d.category,
            name: d.name,
            description: d.description,
            risk: d.risk,
            evidence: d.evidence,
            applied: is_applied(d, &backup),
            admin: needs_admin(d),
            reboot: d.reboot,
        })
        .collect())
}

fn write_entry(e: &RegEntry) -> Result<(), String> {
    let (key, _) = root(e.hive).create_subkey(e.path).map_err(|er| format!("{}: {er}", e.path))?;
    match e.value {
        Val::Dword(v) => key.set_value(e.name, &v),
        Val::Str(v) => key.set_value(e.name, &v),
    }
    .map_err(|er| format!("{}\\{}: {er}", e.path, e.name))
}

fn restore(entries: &[SavedEntry]) -> Result<(), String> {
    for e in entries {
        match &e.orig {
            SavedVal::Missing => {
                if let Ok(key) = root(e.hive).open_subkey_with_flags(&e.path, KEY_SET_VALUE) {
                    let _ = key.delete_value(&e.name);
                }
            }
            SavedVal::Dword(v) => {
                let (key, _) = root(e.hive).create_subkey(&e.path).map_err(|er| er.to_string())?;
                key.set_value(&e.name, v).map_err(|er| er.to_string())?;
            }
            SavedVal::Str(v) => {
                let (key, _) = root(e.hive).create_subkey(&e.path).map_err(|er| er.to_string())?;
                key.set_value(&e.name, v).map_err(|er| er.to_string())?;
            }
        }
    }
    Ok(())
}

pub fn apply(id: &str) -> Result<(), String> {
    let def = find(id)?;
    if needs_admin(def) && !is_admin() {
        return Err("Este ajuste exige administrador. Feche o app e abra-o como administrador.".into());
    }
    let mut backup = load_backup()?;
    if is_applied(def, &backup) {
        return Ok(());
    }
    match &def.kind {
        Kind::Registry(entries) => {
            let active: Vec<&RegEntry> = entries.iter().filter(|e| !skipped(e)).collect();
            if active.is_empty() {
                return Err("nada a aplicar: os itens deste ajuste não existem neste Windows".into());
            }
            let saved: Vec<SavedEntry> = active
                .iter()
                .map(|e| SavedEntry {
                    hive: e.hive,
                    path: e.path.into(),
                    name: e.name.into(),
                    orig: read_saved(e.hive, e.path, e.name),
                })
                .collect();
            backup.insert(id.into(), Saved::Reg(saved.clone()));
            save_backup(&backup)?; // backup ANTES de alterar
            let result = active.iter().try_for_each(|e| write_entry(e));
            if let Err(err) = result {
                // Falha no meio: desfaz o que já foi escrito para não deixar estado parcial.
                let _ = restore(&saved);
                backup.remove(id);
                let _ = save_backup(&backup);
                return Err(err);
            }
        }
        Kind::PowerNexus => {
            let original = parse_guid(&powercfg(&["/getactivescheme"])?)
                .ok_or("não consegui ler o plano de energia atual")?;
            // Parte do Ultimate Performance (ou do Alto Desempenho, se o PC não o oferecer).
            let created = parse_guid(&powercfg(&["-duplicatescheme", ULTIMATE_SCHEME]).or_else(|_| powercfg(&["-duplicatescheme", HIGH_SCHEME]))?)
                .ok_or("não consegui criar o plano de energia")?;
            backup.insert(id.into(), Saved::Power { original, created: created.clone() });
            save_backup(&backup)?;
            let _ = powercfg(&["-changename", &created, "Nexus Performance", "Plano do Nexus Optimizer"]);
            // Cada ajuste é opcional: planos de alguns PCs não têm todas as opções.
            for args in [
                ["/setacvalueindex", created.as_str(), "SUB_PROCESSOR", "PROCTHROTTLEMIN", "100"],
                ["/setacvalueindex", created.as_str(), "SUB_PROCESSOR", "PROCTHROTTLEMAX", "100"],
                ["/setacvalueindex", created.as_str(), "SUB_PROCESSOR", "PERFBOOSTMODE", "2"],
                ["/setacvalueindex", created.as_str(), "SUB_PCIEXPRESS", "ASPM", "0"],
                ["/setacvalueindex", created.as_str(), "SUB_DISK", "DISKIDLE", "0"],
            ] {
                let a: Vec<&str> = args.to_vec();
                let _ = powercfg(&a);
            }
            powercfg(&["/setactive", &created])?;
        }
        Kind::PowerUltimate => {
            let original = parse_guid(&powercfg(&["/getactivescheme"])?)
                .ok_or("não consegui ler o plano de energia atual")?;
            let created = parse_guid(&powercfg(&["-duplicatescheme", ULTIMATE_SCHEME])?)
                .ok_or("não consegui criar o plano Ultimate Performance")?;
            backup.insert(id.into(), Saved::Power { original, created: created.clone() });
            save_backup(&backup)?;
            powercfg(&["/setactive", &created])?;
        }
    }
    Ok(())
}

pub fn revert(id: &str) -> Result<(), String> {
    let def = find(id)?;
    if needs_admin(def) && !is_admin() {
        return Err("Reverter este ajuste exige administrador. Abra o app como administrador.".into());
    }
    let mut backup = load_backup()?;
    let saved = backup.get(id).cloned().ok_or("sem backup para este ajuste; nada a reverter")?;
    match saved {
        Saved::Reg(entries) => restore(&entries)?,
        Saved::Power { original, created } => {
            powercfg(&["/setactive", &original])?;
            let _ = powercfg(&["/delete", &created]);
        }
    }
    backup.remove(id);
    save_backup(&backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isolate() {
        // SAFETY: testes só leem essa variável depois de definida aqui, com o mesmo valor.
        unsafe { std::env::set_var("NEXUS_DATA_DIR", std::env::temp_dir().join("nexus_test_data")) };
    }

    #[test]
    fn catalog_is_consistent() {
        isolate();
        let mut ids: Vec<_> = DEFS.iter().map(|d| d.id).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids duplicados");
        assert!(n >= 25);
        // leitura do estado real deve funcionar sem erro mesmo sem administrador
        assert_eq!(list().unwrap().len(), n);
        for d in DEFS {
            assert!(["seguro", "moderado", "avançado"].contains(&d.risk), "{}", d.id);
        }
    }

    #[test]
    fn old_backup_without_hive_still_loads() {
        let json = r#"{"x":{"Reg":[{"path":"Software\\A","name":"B","orig":{"Dword":1}}]}}"#;
        let b: Backup = serde_json::from_str(json).unwrap();
        match &b["x"] {
            Saved::Reg(v) => assert_eq!(v[0].hive, Hive::Cu),
            _ => panic!(),
        }
    }

    // Aplica e reverte um ajuste HKCU real e confere que o estado original volta.
    #[test]
    fn registry_tweak_roundtrip() {
        isolate();
        let def = find("gamebar-popups-off").unwrap();
        if is_applied(def, &load_backup().unwrap()) {
            eprintln!("gamebar-popups-off já ativo no sistema; teste pulado para não alterar seu estado");
            return;
        }
        let before = read_saved(CU, r"Software\Microsoft\GameBar", "ShowStartupPanel");
        apply("gamebar-popups-off").unwrap();
        assert!(list().unwrap().iter().any(|t| t.id == "gamebar-popups-off" && t.applied));
        revert("gamebar-popups-off").unwrap();
        let after = read_saved(CU, r"Software\Microsoft\GameBar", "ShowStartupPanel");
        assert_eq!(
            serde_json::to_string(&before).unwrap(),
            serde_json::to_string(&after).unwrap()
        );
        assert!(revert("gamebar-popups-off").is_err());
    }
}
