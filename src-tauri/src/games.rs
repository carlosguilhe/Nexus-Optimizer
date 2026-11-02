use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Catálogo público de jogos detectáveis (nome do executável + ícone), o mesmo que o Discord usa
/// para mostrar "jogando X". Baixado uma vez, guardado em disco e renovado de tempos em tempos.
const CATALOG_URL: &str = "https://discord.com/api/v10/applications/detectable";
const ICON_URL: &str = "https://cdn.discordapp.com/app-icons";
const MAX_AGE: Duration = Duration::from_secs(14 * 24 * 3600);

/// Nomes genéricos ou de programas comuns: nunca são tratados como jogo.
const DENY: &[&str] = &[
    "game.exe", "launcher.exe", "client.exe", "main.exe", "start.exe", "setup.exe", "install.exe", "app.exe",
    "chrome.exe", "msedge.exe", "firefox.exe", "opera.exe", "brave.exe", "explorer.exe", "steam.exe",
    "steamwebhelper.exe", "discord.exe", "java.exe", "javaw.exe", "python.exe", "pythonw.exe", "node.exe",
    "cmd.exe", "dosbox.exe", "vlc.exe", "obs64.exe", "obs32.exe", "teams.exe", "code.exe", "spotify.exe",
    "epicgameslauncher.exe", "battle.net.exe", "origin.exe", "ubisoftconnect.exe", "upc.exe", "nexus-optimizer.exe",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub exes: Vec<String>,
}

pub struct Catalog {
    pub games: Vec<Game>,
    index: HashMap<String, usize>,
    pub updated: Option<u64>,
    pub source: &'static str,
}

#[derive(Deserialize)]
struct RawExe {
    name: String,
    os: String,
    #[serde(default)]
    is_launcher: bool,
}

#[derive(Deserialize)]
struct Raw {
    id: String,
    name: String,
    icon_hash: Option<String>,
    #[serde(default)]
    executables: Option<Vec<RawExe>>,
}

static CATALOG: OnceLock<RwLock<Option<Arc<Catalog>>>> = OnceLock::new();

fn cell() -> &'static RwLock<Option<Arc<Catalog>>> {
    CATALOG.get_or_init(|| RwLock::new(None))
}

/// Catálogo já carregado em memória (sem rede nem disco).
pub fn current() -> Option<Arc<Catalog>> {
    cell().read().unwrap().clone()
}

fn set(c: Arc<Catalog>) {
    *cell().write().unwrap() = Some(c);
}

fn normalize_exe(name: &str) -> Option<String> {
    let n = name.trim_start_matches('>').rsplit(['/', '\\']).next()?.to_lowercase();
    (n.ends_with(".exe") && !DENY.contains(&n.as_str())).then_some(n)
}

/// Monta o catálogo. Nomes de executável compartilhados por mais de 2 jogos são ambíguos e ignorados.
fn build(raws: Vec<Raw>, updated: Option<u64>, source: &'static str) -> Catalog {
    let mut owners: HashMap<String, usize> = HashMap::new();
    let mut staged: Vec<(Raw, Vec<String>)> = Vec::new();
    for r in raws {
        let mut exes: Vec<String> = r
            .executables
            .iter()
            .flatten()
            .filter(|e| e.os == "win32" && !e.is_launcher)
            .filter_map(|e| normalize_exe(&e.name))
            .collect();
        exes.sort();
        exes.dedup();
        if exes.is_empty() {
            continue;
        }
        for e in &exes {
            *owners.entry(e.clone()).or_default() += 1;
        }
        staged.push((r, exes));
    }
    let games: Vec<Game> = staged
        .into_iter()
        .filter_map(|(r, exes)| {
            let exes: Vec<String> = exes.into_iter().filter(|e| owners[e] <= 2).collect();
            (!exes.is_empty()).then_some(Game { id: r.id, name: r.name, icon: r.icon_hash, exes })
        })
        .collect();
    finish(games, updated, source)
}

fn finish(games: Vec<Game>, updated: Option<u64>, source: &'static str) -> Catalog {
    let mut index = HashMap::new();
    for (i, g) in games.iter().enumerate() {
        for e in &g.exes {
            index.entry(e.clone()).or_insert(i);
        }
    }
    Catalog { games, index, updated, source }
}

fn builtin() -> Catalog {
    let list: &[(&str, &[&str])] = &[
        ("VALORANT", &["valorant-win64-shipping.exe"]),
        ("Counter-Strike 2", &["cs2.exe"]),
        ("League of Legends", &["league of legends.exe"]),
        ("Fortnite", &["fortniteclient-win64-shipping.exe"]),
        ("Apex Legends", &["r5apex.exe", "r5apex_dx12.exe"]),
        ("Overwatch 2", &["overwatch.exe"]),
        ("Rocket League", &["rocketleague.exe"]),
        ("Grand Theft Auto V", &["gta5.exe", "gta5_enhanced.exe"]),
        ("Rust", &["rustclient.exe"]),
        ("PUBG: BATTLEGROUNDS", &["tslgame.exe"]),
        ("Rainbow Six Siege", &["rainbowsix.exe", "rainbowsix_vulkan.exe"]),
        ("Dota 2", &["dota2.exe"]),
        ("Cyberpunk 2077", &["cyberpunk2077.exe"]),
        ("Elden Ring", &["eldenring.exe"]),
        ("Roblox", &["robloxplayerbeta.exe"]),
        ("Call of Duty", &["cod.exe", "modernwarfare.exe"]),
        ("The Witcher 3", &["witcher3.exe"]),
        ("Destiny 2", &["destiny2.exe"]),
    ];
    let games = list
        .iter()
        .enumerate()
        .map(|(i, (n, e))| Game {
            id: format!("builtin-{i}"),
            name: n.to_string(),
            icon: None,
            exes: e.iter().map(|s| s.to_string()).collect(),
        })
        .collect();
    finish(games, None, "embutido")
}

fn cache_path() -> Result<std::path::PathBuf, String> {
    Ok(crate::tweaks::data_dir()?.join("games.json"))
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load_cache() -> Option<Catalog> {
    let path = cache_path().ok()?;
    let games: Vec<Game> = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
    let updated = std::fs::metadata(&path).and_then(|m| m.modified()).ok().map(unix);
    Some(finish(games, updated, "cache"))
}

fn is_stale(c: &Catalog) -> bool {
    c.updated.map(|u| unix(SystemTime::now()).saturating_sub(u) > MAX_AGE.as_secs()).unwrap_or(true)
}

async fn download() -> Result<Catalog, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .user_agent("NexusOptimizer/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(CATALOG_URL).send().await.map_err(|e| format!("sem acesso ao catálogo de jogos: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("catálogo de jogos indisponível (HTTP {})", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let raws: Vec<Raw> = serde_json::from_slice(&bytes).map_err(|e| format!("catálogo em formato inesperado: {e}"))?;
    let cat = build(raws, Some(unix(SystemTime::now())), "online");
    if let (Ok(path), Ok(json)) = (cache_path(), serde_json::to_vec(&cat.games)) {
        let _ = std::fs::write(path, json);
    }
    Ok(cat)
}

/// Garante um catálogo carregado: memória → cache em disco (se recente) → internet → cache velho → lista embutida.
pub async fn ensure(refresh: bool) -> Arc<Catalog> {
    if !refresh {
        if let Some(c) = current() {
            return c;
        }
    }
    let cached = if refresh { None } else { load_cache() };
    if let Some(c) = &cached {
        if !is_stale(c) {
            let c = Arc::new(cached.unwrap());
            set(c.clone());
            return c;
        }
    }
    match download().await {
        Ok(c) => {
            let c = Arc::new(c);
            set(c.clone());
            c
        }
        Err(_) => {
            let c = Arc::new(cached.or_else(load_cache).unwrap_or_else(builtin));
            set(c.clone());
            c
        }
    }
}

#[derive(Serialize)]
pub struct CatalogStatus {
    pub count: usize,
    pub updated: Option<u64>,
    pub source: &'static str,
}

pub fn status(c: &Catalog) -> CatalogStatus {
    CatalogStatus { count: c.games.len(), updated: c.updated, source: c.source }
}

pub struct Running {
    pub game: usize,
    pub exe: String,
    pub pids: Vec<u32>,
    pub mem_bytes: u64,
}

/// Jogos em execução agora, do que mais usa memória para o que menos usa.
pub fn detect(sys: &sysinfo::System, cat: &Catalog) -> Vec<Running> {
    let mut by_exe: HashMap<String, Running> = HashMap::new();
    for (pid, p) in sys.processes() {
        let name = p.name().to_string_lossy().to_lowercase();
        let Some(&gi) = cat.index.get(&name) else { continue };
        // Ignora processos do Windows que por coincidência tenham o mesmo nome.
        if p.exe().is_some_and(|e| e.to_string_lossy().to_lowercase().starts_with(&crate::sys::windows_dir())) {
            continue;
        }
        let r = by_exe.entry(name.clone()).or_insert(Running { game: gi, exe: name, pids: vec![], mem_bytes: 0 });
        r.pids.push(pid.as_u32());
        r.mem_bytes += p.memory();
    }
    let mut v: Vec<Running> = by_exe.into_values().collect();
    // Vários executáveis do mesmo jogo (jogo + anticheat): mantém o que mais usa memória.
    v.sort_by(|a, b| b.mem_bytes.cmp(&a.mem_bytes));
    let mut seen = HashSet::new();
    v.retain(|r| seen.insert(r.game));
    v
}

#[derive(Serialize)]
pub struct GameHit {
    pub name: String,
    pub process: String,
    pub icon_url: Option<String>,
}

/// Jogos do catálogo cujo nome contém o texto (sem diferenciar maiúsculas), os que começam com ele primeiro.
pub fn search(query: &str, limit: usize) -> Vec<GameHit> {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return vec![];
    }
    let Some(cat) = current() else { return vec![] };
    let mut hits: Vec<(&Game, bool)> = cat
        .games
        .iter()
        .filter(|g| g.name.to_lowercase().contains(&q))
        .map(|g| (g, g.name.to_lowercase().starts_with(&q)))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.name.len().cmp(&b.0.name.len())));
    hits.into_iter()
        .take(limit)
        .map(|(g, _)| GameHit {
            name: g.name.clone(),
            process: g.exes[0].clone(),
            icon_url: g.icon.as_ref().map(|h| format!("{ICON_URL}/{}/{h}.png?size=64", g.id)),
        })
        .collect()
}

#[derive(Serialize)]
pub struct RunningGame {
    pub process: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub mem_bytes: u64,
}

pub fn running_games() -> Vec<RunningGame> {
    let Some(cat) = current() else { return vec![] };
    crate::sys::with_processes(|s| detect(s, &cat))
        .into_iter()
        .map(|r| {
            let g = &cat.games[r.game];
            RunningGame {
                process: r.exe,
                name: g.name.clone(),
                icon_url: g.icon.as_ref().map(|h| format!("{ICON_URL}/{}/{h}.png?size=64", g.id)),
                mem_bytes: r.mem_bytes,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: &str, name: &str, exes: &[(&str, bool)]) -> Raw {
        Raw {
            id: id.into(),
            name: name.into(),
            icon_hash: Some("h".into()),
            executables: Some(
                exes.iter().map(|(n, l)| RawExe { name: n.to_string(), os: "win32".into(), is_launcher: *l }).collect(),
            ),
        }
    }

    #[test]
    fn exe_names_are_normalized_and_filtered() {
        assert_eq!(normalize_exe(">Win64/CS2.exe").as_deref(), Some("cs2.exe"));
        assert_eq!(normalize_exe("game.exe"), None, "genérico");
        assert_eq!(normalize_exe("chrome.exe"), None, "navegador");
        assert_eq!(normalize_exe("jogo.app"), None, "não é Windows");
    }

    #[test]
    fn catalog_drops_launchers_and_ambiguous_exes() {
        let cat = build(
            vec![
                raw("1", "Jogo A", &[("a.exe", false), ("a_launcher.exe", true)]),
                raw("2", "Mod 1", &[("hl2.exe", false)]),
                raw("3", "Mod 2", &[("hl2.exe", false)]),
                raw("4", "Mod 3", &[("hl2.exe", false)]),
            ],
            None,
            "teste",
        );
        assert_eq!(cat.games.len(), 1);
        assert_eq!(cat.games[0].exes, vec!["a.exe"]);
        assert!(cat.index.contains_key("a.exe") && !cat.index.contains_key("hl2.exe"));
    }

    #[test]
    fn builtin_matches_known_games() {
        let c = builtin();
        assert!(c.index.contains_key("valorant-win64-shipping.exe") && c.index.contains_key("cs2.exe"));
    }

    // Usa a internet (catálogo de ~13 MB): `cargo test real_catalog -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_catalog() {
        let c = tauri::async_runtime::block_on(download()).unwrap();
        println!("{} jogos, {} executáveis indexados", c.games.len(), c.index.len());
        assert!(c.games.len() > 5000);
        assert!(c.index.contains_key("valorant-win64-shipping.exe"));
        for bad in ["explorer.exe", "chrome.exe", "svchost.exe", "code.exe", "java.exe", "node.exe", "notepad.exe"] {
            assert!(!c.index.contains_key(bad), "{bad} não deveria ser jogo");
        }
        let found = crate::sys::with_processes(|s| detect(s, &c));
        println!("detectados agora: {}", found.len());
        for r in found {
            println!("  {} -> {} ({} proc)", r.exe, c.games[r.game].name, r.pids.len());
        }
    }
}
