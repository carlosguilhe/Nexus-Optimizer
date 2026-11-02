import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Power, Search, ShieldAlert } from "lucide-react";
import "./App.css";
import { errText, Tweak } from "./types";
import { useToast } from "./toast";
import { useConfirm } from "./confirm";
import { useT } from "./i18n";
import { useSettings } from "./settings";
import { GROUPS, TABS, tabIcon } from "./icons";
import { Preset } from "./presets";
import { TweakList } from "./components/TweakList";
import { CleanTool, DnsTool, GameMode, GameModeCard, PingTool, PriorityTool, RestoreCard } from "./components/Tools";
import { AffinityTool, MsiCard, ProfilesCard } from "./components/Profiles";
import { DnsBench, LivePing, RouteTrace, ServersCard } from "./components/NetTools";
import { PageHeader, ProgressBar, Spinner } from "./components/ui";
import { Splash } from "./components/Splash";
import { Wizard } from "./components/Wizard";
import { Home } from "./tabs/Home";
import { BenchTab } from "./tabs/BenchTab";
import { SpeedTab } from "./tabs/SpeedTab";
import { ProcessesTab } from "./tabs/ProcessesTab";
import { StartupTab } from "./tabs/StartupTab";
import { AppsTab } from "./tabs/AppsTab";
import { MonitorTab } from "./tabs/MonitorTab";
import { MaintTab } from "./tabs/MaintTab";
import { SettingsTab } from "./tabs/SettingsTab";
import { HelpTab } from "./tabs/HelpTab";

// Abas com conteúdo próprio, sem a lista de ajustes do catálogo.
const CUSTOM_TABS = ["Início", "Velocidade", "Aplicativos", "Inicialização", "Processos", "Benchmark", "Monitor", "Manutenção", "Configurações", "Ajuda"];

const TAB_INFO: Record<string, string> = {
  Jogos: "Ajustes do Windows que afetam o desempenho e a latência em jogos.",
  Energia: "Plano de energia e comportamento de desligamento.",
  Entrada: "Mouse e teclado: precisão e responsividade.",
  Rede: "Ajustes de rede, latência, DNS e rota.",
  Visual: "Efeitos visuais do Windows. Faz mais diferença em PCs fracos.",
  Privacidade: "Telemetria, anúncios e processos em segundo plano.",
  Serviços: "Serviços do Windows desativados via registro. Exigem reiniciar.",
  Sistema: "Desligamento, busca, notificações, limpeza e ponto de restauração.",
  Avançado: "Ajustes que trocam segurança por desempenho. Só use se medir ganho real.",
};

const TAB_KEY = "nexus.tab";
const SPLASH_MIN_MS = 2300;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function savedTab(): string {
  try {
    const v = localStorage.getItem(TAB_KEY);
    return v && TABS.includes(v) ? v : "Início";
  } catch {
    return "Início";
  }
}

interface Bulk {
  label: string;
  done: number;
  total: number;
}

function App() {
  const t = useT();
  const notify = useToast();
  const confirm = useConfirm();
  const { settings, loaded, update } = useSettings();
  const [tab, setTabState] = useState(savedTab);
  const [allTweaks, setTweaks] = useState<Tweak[]>([]);
  // Ajustes do Windows Update ficam fora das abas e da busca: têm cartão próprio em Manutenção.
  const tweaks = useMemo(() => allTweaks.filter((x) => TABS.includes(x.category)), [allTweaks]);
  const updateTweaks = useMemo(() => allTweaks.filter((x) => !TABS.includes(x.category)), [allTweaks]);
  const [admin, setAdmin] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [bulk, setBulk] = useState<Bulk | null>(null);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [gmLive, setGmLive] = useState<{ active: boolean; game: string | null }>({ active: false, game: null });
  const [autostart, setAutostart] = useState(false);
  const [autostartBusy, setAutostartBusy] = useState(false);
  const [newVersion, setNewVersion] = useState<{ latest: string } | null>(null);
  const [boot, setBoot] = useState({ done: 0, total: 4, label: "Iniciando…" });
  const [splash, setSplash] = useState<"show" | "leaving" | "gone">("show");
  const booted = useRef(false);
  const searchRef = useRef<HTMLInputElement>(null);

  const gm: GameMode = { enabled: settings.auto_game_mode, active: gmLive.active, game: gmLive.game };

  function setTab(name: string) {
    setTabState(name);
    setQuery("");
    try {
      localStorage.setItem(TAB_KEY, name);
    } catch {
      /* sem armazenamento: só não lembra a aba */
    }
  }

  const refresh = useCallback(async () => {
    try {
      setTweaks(await invoke<Tweak[]>("list_tweaks"));
    } catch (e) {
      setError(errText(e));
    }
  }, []);

  // Carregamento inicial, mostrado na tela de abertura.
  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    (async () => {
      const t0 = Date.now();
      const steps: [string, () => Promise<unknown>][] = [
        ["Carregando ajustes…", refresh],
        ["Verificando permissões…", async () => setAdmin(await invoke<boolean>("is_admin"))],
        [
          "Lendo configurações…",
          async () => {
            const g = await invoke<{ active: boolean; game: string | null }>("gamemode_get");
            setGmLive({ active: g.active, game: g.game });
            setAutostart(await invoke<boolean>("autostart_get"));
          },
        ],
        ["Preparando catálogo de jogos…", () => Promise.race([invoke("catalog_status", { refresh: false }), sleep(4000)])],
      ];
      for (let i = 0; i < steps.length; i++) {
        setBoot({ done: i, total: steps.length, label: steps[i][0] });
        try {
          await steps[i][1]();
        } catch {
          /* uma etapa que falha não impede o app de abrir */
        }
      }
      setBoot({ done: steps.length, total: steps.length, label: "Pronto" });
      await sleep(Math.max(300, SPLASH_MIN_MS - (Date.now() - t0)));
      setSplash("leaving");
      await sleep(650);
      setSplash("gone");
    })();
  }, [refresh]);

  useEffect(() => {
    const a = listen<{ active: boolean; game: string | null }>("gamemode", (e) => setGmLive(e.payload));
    const b = listen<{ latest: string }>("update", (e) => setNewVersion(e.payload));
    return () => {
      a.then((f) => f());
      b.then((f) => f());
    };
  }, []);

  // Ctrl+K foca a busca.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  async function toggleAutostart() {
    const next = !autostart;
    setAutostartBusy(true);
    try {
      await invoke("autostart_set", { enabled: next });
      setAutostart(next);
      notify(next ? t("O Nexus vai iniciar junto com o Windows.") : t("O Nexus não inicia mais com o Windows."));
    } catch (e) {
      setError(t(errText(e)));
    }
    setAutostartBusy(false);
  }

  async function setGameMode(on: boolean) {
    try {
      await invoke("gamemode_set", { enabled: on });
      notify(on ? t("Modo Jogo automático ligado.") : t("Modo Jogo automático desligado."));
    } catch (e) {
      setError(t(errText(e)));
    }
  }

  async function toggle(tw: Tweak) {
    setBusy(tw.id);
    setError("");
    try {
      await invoke(tw.applied ? "revert_tweak" : "apply_tweak", { id: tw.id });
      notify(`${tw.applied ? t("Revertido") : t("Aplicado")}: ${t(tw.name)}${!tw.applied && tw.reboot ? ` (${t("reinicie para valer")})` : ""}`);
    } catch (e) {
      setError(`${t(tw.name)}: ${t(errText(e))}`);
    }
    await refresh();
    setBusy(null);
  }

  /** Aplica ou reverte vários ajustes em sequência, com barra de progresso. */
  async function runBulk(list: Tweak[], command: "apply_tweak" | "revert_tweak", label: string, doneMsg: string) {
    setBusy("all");
    setError("");
    const errors: string[] = [];
    let ok = 0;
    for (let i = 0; i < list.length; i++) {
      setBulk({ label: `${label} ${t(list[i].name)}`, done: i, total: list.length });
      try {
        await invoke(command, { id: list[i].id });
        ok++;
      } catch (e) {
        errors.push(`${t(list[i].name)}: ${t(errText(e))}`);
      }
    }
    setBulk(null);
    if (errors.length) setError(errors.join("\n"));
    if (ok) notify(t(doneMsg, { n: ok }));
    await refresh();
    setBusy(null);
  }

  const applyAllSafe = () =>
    runBulk(
      tweaks.filter((x) => x.risk === "seguro" && !x.applied && (!x.admin || admin)),
      "apply_tweak",
      t("Aplicando:"),
      "{n} ajuste(s) seguros aplicados.",
    );

  const presetTweaks = (ids: string[]) => tweaks.filter((x) => ids.includes(x.id) && !x.applied && (!x.admin || admin));

  async function applyPreset(p: Preset) {
    const list = presetTweaks(p.tweaks);
    if (!list.length) {
      notify(t("Todos os ajustes deste perfil já estão ativos."));
      return;
    }
    const ok = await confirm({
      title: t("Aplicar o perfil {n}?", { n: t(p.name) }),
      message: t("{n} ajuste(s) serão aplicados. Você pode reverter cada um depois, ou todos de uma vez em Início.", { n: list.length }),
      confirmLabel: t("Aplicar"),
    });
    if (ok) runBulk(list, "apply_tweak", t("Aplicando:"), "{n} ajuste(s) aplicados.");
  }

  async function revertAll() {
    const applied = allTweaks.filter((x) => x.applied && (!x.admin || admin));
    if (!applied.length) return;
    const ok = await confirm({
      title: t("Reverter todos os ajustes?"),
      message: t("{n} ajuste(s) voltarão ao estado original, como estavam antes do Nexus.", { n: applied.length }),
      confirmLabel: t("Reverter tudo"),
    });
    if (ok) runBulk(applied, "revert_tweak", t("Revertendo:"), "{n} ajuste(s) revertidos.");
  }

  async function relaunchAdmin() {
    setError("");
    try {
      await invoke("relaunch_admin");
    } catch (e) {
      setError(t(errText(e)));
    }
  }

  function count(cat: string) {
    const list = tweaks.filter((x) => x.category === cat);
    return list.length ? `${list.filter((x) => x.applied).length}/${list.length}` : "";
  }

  const q = query.trim().toLowerCase();
  const results =
    q.length >= 2 ? tweaks.filter((x) => `${t(x.name)} ${t(x.description)} ${t(x.category)} ${x.name}`.toLowerCase().includes(q)) : null;
  const tabTweaks = tweaks.filter((x) => x.category === tab);
  const showWizard = loaded && !settings.wizard_done && splash === "gone" && tweaks.length > 0;

  return (
    <>
      {splash !== "gone" && <Splash progress={boot.done / boot.total} label={t(boot.label)} leaving={splash === "leaving"} />}
      {showWizard && (
        <Wizard
          tweaks={tweaks}
          admin={admin}
          onApply={(ids) => runBulk(presetTweaks(ids), "apply_tweak", t("Aplicando:"), "{n} ajuste(s) aplicados.")}
          onFinish={() => update({ wizard_done: true })}
        />
      )}
      <div className="app">
        <aside className="sidebar">
          <div className="brand">
            <img className="logo" src="/nexus.svg" alt="" />
            <div>
              <b>NEXUS</b>
              <small>OPTIMIZER</small>
            </div>
          </div>
          <nav>
            {GROUPS.map((g) => (
              <div key={g.label ?? "main"} className="nav-group">
                {g.label && <span className="nav-label">{t(g.label)}</span>}
                {g.tabs.map((name) => {
                  const Icon = tabIcon(name);
                  return (
                    <button key={name} className={`nav ${tab === name && !results ? "active" : ""}`} onClick={() => setTab(name)}>
                      <Icon size={17} />
                      <span>{t(name)}</span>
                      <em>{count(name)}</em>
                    </button>
                  );
                })}
              </div>
            ))}
          </nav>
          {admin ? (
            <div className="status ok">
              <ShieldAlert size={14} /> {t("Administrador")}
            </div>
          ) : (
            <button className="ghost admin-btn" onClick={relaunchAdmin} title={t("Reabre o app com permissões de administrador")}>
              <ShieldAlert size={14} /> {t("Reabrir como administrador")}
            </button>
          )}
        </aside>

        <main className="content">
          <div className="topbar">
            <label className="search-wrap">
              <Search size={16} />
              <input
                ref={searchRef}
                className="search"
                type="search"
                placeholder={t("Buscar ajustes (Ctrl+K)…")}
                value={query}
                onChange={(e) => setQuery(e.currentTarget.value)}
              />
            </label>
            <button
              className={`autostart-btn ${autostart ? "active" : ""}`}
              aria-pressed={autostart}
              disabled={autostartBusy}
              onClick={toggleAutostart}
              title={autostart ? t("O Nexus inicia com o Windows (clique para desativar)") : t("Iniciar o Nexus junto com o Windows")}
            >
              {autostartBusy ? <Spinner size={15} /> : <Power size={15} />}
              {t("Iniciar com o Windows")}
            </button>
          </div>

          {bulk && (
            <section className="card">
              <ProgressBar value={bulk.done / bulk.total} label={`${bulk.label} (${bulk.done + 1}/${bulk.total})`} />
            </section>
          )}
          {gm.active && (
            <p className="banner">
              <i className="dot" /> {t("Modo Jogo ativo:")} <b>{gm.game}</b>
            </p>
          )}
          {error && (
            <p className="error" onClick={() => setError("")} title={t("Clique para fechar")}>
              {error}
            </p>
          )}
          {!admin && (results || !CUSTOM_TABS.includes(tab)) && (
            <p className="warn">{t("Sem administrador: ajustes marcados com \"admin\" ficam desativados. Use \"Reabrir como administrador\" na barra lateral.")}</p>
          )}

          {results ? (
            <>
              <PageHeader icon={Search} title={t("Resultados")} desc={t("{n} ajuste(s) para \"{q}\"", { n: results.length, q: query.trim() })} />
              {results.length === 0 && <p className="empty">{t("Nada encontrado. Tente outra palavra.")}</p>}
              <TweakList tweaks={results} admin={admin} busy={busy} onToggle={toggle} showCategory />
            </>
          ) : (
            <>
              {tab === "Início" && (
                <Home
                  tweaks={tweaks}
                  admin={admin}
                  busy={busy}
                  update={newVersion}
                  onApplySafe={applyAllSafe}
                  onRevertAll={revertAll}
                  onApplyPreset={applyPreset}
                  goTo={setTab}
                />
              )}
              {tab === "Benchmark" && <BenchTab tweaks={tweaks} admin={admin} refresh={refresh} />}
              {tab === "Velocidade" && <SpeedTab />}
              {tab === "Monitor" && <MonitorTab />}
              {tab === "Aplicativos" && <AppsTab />}
              {tab === "Inicialização" && <StartupTab admin={admin} />}
              {tab === "Processos" && <ProcessesTab />}
              {tab === "Manutenção" && <MaintTab admin={admin} updateTweaks={updateTweaks} refresh={refresh} />}
              {tab === "Configurações" && <SettingsTab />}
              {tab === "Ajuda" && <HelpTab />}

              {!CUSTOM_TABS.includes(tab) && (
                <>
                  <PageHeader icon={tabIcon(tab)} title={t(tab)} desc={t(TAB_INFO[tab])} />
                  {tab === "Jogos" && <GameModeCard gm={gm} onChange={setGameMode} />}
                  <TweakList tweaks={tabTweaks} admin={admin} busy={busy} onToggle={toggle} />
                  {tab === "Jogos" && (
                    <>
                      <ProfilesCard tweaks={tweaks} />
                      <PriorityTool />
                      <AffinityTool />
                    </>
                  )}
                  {tab === "Rede" && (
                    <>
                      <ServersCard />
                      <LivePing />
                      <PingTool />
                      <RouteTrace />
                      <DnsBench admin={admin} />
                      <DnsTool admin={admin} />
                    </>
                  )}
                  {tab === "Sistema" && (
                    <>
                      <CleanTool admin={admin} />
                      <RestoreCard admin={admin} />
                    </>
                  )}
                  {tab === "Avançado" && <MsiCard admin={admin} />}
                </>
              )}
            </>
          )}
        </main>
      </div>
    </>
  );
}

export default App;
