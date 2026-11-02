import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileText, FlaskConical, Trash2 } from "lucide-react";
import { errText, fmtBytes, HistoryEntry, Tweak } from "../types";
import { useT } from "../i18n";
import { useToast } from "../toast";
import { useConfirm } from "../confirm";
import { useSettings } from "../settings";
import { GameIcon } from "../components/GameIcon";
import { BtnLabel, Loading, PageHeader, ProgressBar } from "../components/ui";
import { CHART_COLORS, LineChart } from "../components/charts";
import { tabIcon } from "../icons";

interface RunningGame {
  process: string;
  name: string;
  icon_url: string | null;
  mem_bytes: number;
}

interface CatalogStatus {
  count: number;
  updated: number | null;
  source: string;
}

interface Verdict {
  keep: boolean;
  gain_pct: number;
  low_gain_pct: number;
  noise_pct: number;
  reason: string;
}

function delta(before: number, after: number) {
  const d = ((after - before) / before) * 100;
  return `${d >= 0 ? "+" : ""}${d.toFixed(1)}%`;
}

/** Escala do gráfico acompanha os dados (picos incluídos), entre 10 e 100 ms. */
function chartMax(entries: HistoryEntry[]) {
  const peak = Math.max(1, ...entries.flatMap((e) => e.result.series));
  return Math.min(100, Math.max(10, Math.ceil((peak * 1.1) / 5) * 5));
}

interface Props {
  tweaks: Tweak[];
  admin: boolean;
  refresh: () => Promise<void>;
}

export function BenchTab({ tweaks, admin, refresh }: Props) {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const { settings } = useSettings();
  const [games, setGames] = useState<RunningGame[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [catalog, setCatalog] = useState<CatalogStatus | null>(null);
  const [updating, setUpdating] = useState(false);
  const [selected, setSelected] = useState("");
  const [manual, setManual] = useState(false);
  const [seconds, setSeconds] = useState(30);
  const [measuring, setMeasuring] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [err, setErr] = useState("");
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [picked, setPicked] = useState<number[]>([]);
  const [last, setLast] = useState<{ before?: HistoryEntry; after?: HistoryEntry }>({});
  // Teste A/B
  const [abTweak, setAbTweak] = useState("");
  const [abStep, setAbStep] = useState<{ n: number; label: string } | null>(null);
  const [verdict, setVerdict] = useState<Verdict | null>(null);

  const gameName = games.find((g) => g.process === selected)?.name ?? selected;

  const loadHistory = useCallback(() => invoke<HistoryEntry[]>("bench_history").then(setHistory).catch(() => {}), []);

  const load = useCallback(async () => {
    try {
      const g = await invoke<RunningGame[]>("running_games");
      setGames(g);
      setSelected((cur) => (g.some((x) => x.process === cur) || manual ? cur : g.length === 1 ? g[0].process : ""));
    } catch (e) {
      setErr(t(errText(e)));
    }
    setLoaded(true);
  }, [manual, t]);

  useEffect(() => {
    load();
    const id = setInterval(load, 4000);
    return () => clearInterval(id);
  }, [load]);

  useEffect(() => {
    invoke<CatalogStatus>("catalog_status", { refresh: false }).then(setCatalog).catch(() => {});
    loadHistory();
  }, [loadHistory]);

  // O comando bloqueia pela duração configurada; a barra avança com o relógio.
  useEffect(() => {
    if (!measuring) return;
    const start = Date.now();
    setElapsed(0);
    const id = setInterval(() => setElapsed((Date.now() - start) / 1000), 200);
    return () => clearInterval(id);
  }, [measuring]);

  async function updateCatalog() {
    setUpdating(true);
    try {
      setCatalog(await invoke<CatalogStatus>("catalog_status", { refresh: true }));
      await load();
    } catch (e) {
      setErr(t(errText(e)));
    }
    setUpdating(false);
  }

  async function run(label: string): Promise<HistoryEntry | null> {
    setMeasuring(label);
    setErr("");
    try {
      const e = await invoke<HistoryEntry>("run_benchmark", { process: selected, game: gameName, label, seconds });
      await loadHistory();
      return e;
    } catch (e) {
      setErr(t(errText(e)));
      return null;
    } finally {
      setMeasuring(null);
    }
  }

  async function measure(slot: "before" | "after") {
    const e = await run(slot === "before" ? "antes" : "depois");
    if (e) setLast((l) => ({ ...l, [slot]: e }));
  }

  async function runAb() {
    const tw = tweaks.find((x) => x.id === abTweak);
    if (!tw) return;
    const ok = await confirm({
      title: t("Testar {n}?", { n: t(tw.name) }),
      message: t("O Nexus vai medir {s}s duas vezes, aplicar o ajuste, medir mais duas vezes e manter o ajuste só se o ganho for real. Total: cerca de {m} minutos. Mantenha o jogo na mesma cena.", { s: seconds, m: Math.ceil((seconds * 4) / 60) + 1 }),
      confirmLabel: t("Começar"),
    });
    if (!ok) return;
    setVerdict(null);
    const before: HistoryEntry[] = [];
    const after: HistoryEntry[] = [];
    try {
      for (let i = 0; i < 2; i++) {
        setAbStep({ n: i + 1, label: t("Medindo sem o ajuste ({i}/2)", { i: i + 1 }) });
        const e = await run("ab-antes");
        if (!e) return;
        before.push(e);
      }
      setAbStep({ n: 3, label: t("Aplicando o ajuste…") });
      await invoke("apply_tweak", { id: tw.id });
      await refresh();
      for (let i = 0; i < 2; i++) {
        setAbStep({ n: 3 + i + 1, label: t("Medindo com o ajuste ({i}/2)", { i: i + 1 }) });
        const e = await run("ab-depois");
        if (!e) {
          await invoke("revert_tweak", { id: tw.id }).catch(() => {});
          await refresh();
          return;
        }
        after.push(e);
      }
      const v = await invoke<Verdict>("ab_verdict", { before: before.map((e) => e.result), after: after.map((e) => e.result) });
      setVerdict(v);
      if (!v.keep) {
        await invoke("revert_tweak", { id: tw.id });
        toast(t("Ajuste revertido: sem ganho comprovado."));
      } else {
        toast(t("Ajuste mantido: ganho comprovado."));
      }
      await refresh();
    } finally {
      setAbStep(null);
    }
  }

  async function makeReport() {
    try {
      const path = await invoke<string>("bench_report", { ids: picked });
      await invoke("open_report", { path });
      toast(t("Relatório aberto no navegador."));
    } catch (e) {
      toast(t(errText(e)));
    }
  }

  async function removePicked() {
    for (const id of picked) await invoke("bench_delete", { id });
    setPicked([]);
    loadHistory();
  }

  const busy = measuring !== null;
  const when = catalog?.updated ? new Date(catalog.updated * 1000).toLocaleDateString(settings.lang) : null;
  const chosen = history.filter((h) => picked.includes(h.id));
  const abCandidates = tweaks.filter((x) => !x.reboot && (!x.admin || admin));
  const labelName: Record<string, string> = { antes: t("antes"), depois: t("depois"), manual: t("manual"), "ab-antes": t("A/B sem"), "ab-depois": t("A/B com") };

  return (
    <>
      <PageHeader
        icon={tabIcon("Benchmark")}
        title="Benchmark"
        desc={t("Abra o jogo numa cena repetível. Os jogos em execução aparecem abaixo; escolha um e meça antes e depois de aplicar ajustes. Exige administrador.")}
      />

      <section className="card">
        <div className="row spread">
          <h3>{t("Jogos detectados")}</h3>
          <small className="muted">
            {catalog ? t("Catálogo: {n} jogos ({src})", { n: catalog.count.toLocaleString(settings.lang), src: catalog.source + (when ? `, ${when}` : "") }) : t("Carregando catálogo…")}
          </small>
        </div>
        {!loaded && <Loading text={t("Procurando jogos em execução…")} />}
        {updating && <ProgressBar label={t("Baixando o catálogo de jogos…")} />}
        {loaded && games.length === 0 && <p className="empty">{t("Nenhum jogo detectado. Abra o jogo e aguarde alguns segundos; esta lista atualiza sozinha.")}</p>}
        <ul className="game-list">
          {games.map((g) => (
            <li key={g.process}>
              <button className={`game ${selected === g.process && !manual ? "selected" : ""}`} onClick={() => { setManual(false); setSelected(g.process); }}>
                <GameIcon url={g.icon_url} name={g.name} />
                <span className="game-text">
                  <b>{g.name}</b>
                  <small>{g.process} · {fmtBytes(g.mem_bytes)}</small>
                </span>
              </button>
            </li>
          ))}
        </ul>
        <div className="row">
          <button className="ghost small" onClick={updateCatalog} disabled={updating}>
            <BtnLabel busy={updating}>{updating ? t("Atualizando…") : t("Atualizar catálogo")}</BtnLabel>
          </button>
          <button className="ghost small" onClick={() => setManual((m) => !m)}>
            {manual ? t("Usar lista") : t("Meu jogo não aparece")}
          </button>
        </div>
        {manual && (
          <div className="row">
            <input value={selected} onChange={(e) => setSelected(e.currentTarget.value)} placeholder={t("ex.: VALORANT-Win64-Shipping.exe (Gerenciador de Tarefas > Detalhes)")} />
          </div>
        )}
      </section>

      <section className="card">
        <div className="row">
          <span className="muted">{t("Duração")}</span>
          <input type="number" min={5} max={300} value={seconds} onChange={(e) => setSeconds(Number(e.currentTarget.value))} />
          <span className="muted">s</span>
          <button disabled={busy || !selected} onClick={() => measure("before")}>
            <BtnLabel busy={measuring === "antes"}>{measuring === "antes" ? t("Medindo…") : t("Medir ANTES")}</BtnLabel>
          </button>
          <button className="ghost" disabled={busy || !selected} onClick={() => measure("after")}>
            <BtnLabel busy={measuring === "depois"}>{t("Medir DEPOIS")}</BtnLabel>
          </button>
        </div>
        {measuring && (
          <ProgressBar value={Math.min(1, elapsed / (seconds + 2))} label={t("Medindo… {a}/{b} s (mantenha o jogo na mesma cena)", { a: Math.min(Math.round(elapsed), seconds), b: seconds })} />
        )}
        {!selected && <p className="muted">{t("Selecione um jogo acima para medir.")}</p>}
        {err && <p className="error" onClick={() => setErr("")}>{err}</p>}

        {last.before && (
          <>
            <table>
              <thead>
                <tr><th></th><th>{t("Antes")}</th><th>{t("Depois")}</th><th>{t("Variação")}</th></tr>
              </thead>
              <tbody>
                <tr>
                  <td>{t("FPS médio")}</td>
                  <td>{last.before.result.avg_fps.toFixed(1)}</td>
                  <td>{last.after ? last.after.result.avg_fps.toFixed(1) : "—"}</td>
                  <td className="delta">{last.after ? delta(last.before.result.avg_fps, last.after.result.avg_fps) : ""}</td>
                </tr>
                <tr>
                  <td>1% low</td>
                  <td>{last.before.result.low_1pct_fps.toFixed(1)}</td>
                  <td>{last.after ? last.after.result.low_1pct_fps.toFixed(1) : "—"}</td>
                  <td className="delta">{last.after ? delta(last.before.result.low_1pct_fps, last.after.result.low_1pct_fps) : ""}</td>
                </tr>
                <tr>
                  <td>{t("Maior frametime (ms)")}</td>
                  <td>{last.before.result.max_frametime_ms.toFixed(1)}</td>
                  <td>{last.after ? last.after.result.max_frametime_ms.toFixed(1) : "—"}</td>
                  <td></td>
                </tr>
              </tbody>
            </table>
            <LineChart
              series={[last.before, ...(last.after ? [last.after] : [])].map((e, i) => ({ values: e.result.series, color: CHART_COLORS[i] }))}
              max={chartMax([last.before, ...(last.after ? [last.after] : [])])}
              guides={[{ y: 16.7, label: "60 FPS" }, { y: 8.3, label: "120 FPS" }]}
            />
          </>
        )}
        {last.before && last.after && <p className="muted">{t("Uma única medição varia de uma corrida para outra (± alguns %). Repita a mesma cena algumas vezes antes de concluir que um ajuste ajudou.")}</p>}
      </section>

      <section className="card">
        <h3><FlaskConical size={14} /> {t("Teste A/B guiado")}</h3>
        <p className="muted">
          {t("Mede duas vezes sem o ajuste e duas com ele, calcula o ruído do próprio teste e só mantém o ajuste se o ganho passar disso. Acaba com o \"será que ajudou?\".")}
        </p>
        <div className="row">
          <select value={abTweak} onChange={(e) => setAbTweak(e.currentTarget.value)} disabled={busy}>
            <option value="">{t("Escolha o ajuste a testar…")}</option>
            {abCandidates.filter((x) => !x.applied).map((x) => (
              <option key={x.id} value={x.id}>{t(x.name)}</option>
            ))}
          </select>
          <button disabled={busy || !abTweak || !selected || !admin} onClick={runAb}>
            <BtnLabel busy={abStep !== null}>{t("Começar teste")}</BtnLabel>
          </button>
        </div>
        {!admin && <p className="warn">{t("Exige administrador (o benchmark usa o PresentMon).")}</p>}
        {abStep && <ProgressBar value={(abStep.n - 1) / 6} label={`${abStep.label} · ${t("etapa")} ${abStep.n}/6`} />}
        {verdict && (
          <p className={verdict.keep ? "ok" : "warn"}>
            <b>{verdict.keep ? t("Ajuste mantido.") : t("Ajuste revertido.")}</b> FPS {verdict.gain_pct >= 0 ? "+" : ""}{verdict.gain_pct.toFixed(1)}% · 1% low {verdict.low_gain_pct >= 0 ? "+" : ""}{verdict.low_gain_pct.toFixed(1)}% · {t("ruído")} {verdict.noise_pct.toFixed(1)}%
            <br />
            {verdict.keep
              ? t("Ganho acima do ruído do teste e sem piorar o 1% low.")
              : verdict.gain_pct > 3 && verdict.low_gain_pct < -3
                ? t("O FPS médio subiu, mas o 1% low piorou.")
                : t("O ganho não passou do ruído do teste.")}
          </p>
        )}
      </section>

      <section className="card">
        <div className="row spread">
          <h3>{t("Histórico de medições")}</h3>
          <div className="row">
            <button className="ghost small" disabled={!picked.length} onClick={makeReport}>
              <FileText size={13} /> {t("Gerar relatório")}
            </button>
            <button className="ghost small" disabled={!picked.length} onClick={removePicked}>
              <Trash2 size={13} /> {t("Excluir")}
            </button>
            <button className="ghost small" onClick={() => invoke("open_reports_folder")}>{t("Pasta de relatórios")}</button>
          </div>
        </div>
        {history.length === 0 && <p className="empty">{t("Nenhuma medição ainda. Cada medição fica salva aqui para você comparar depois.")}</p>}
        {chosen.length > 0 && (
          <LineChart
            series={chosen.map((e, i) => ({ values: e.result.series, color: CHART_COLORS[i % 4] }))}
            max={chartMax(chosen)}
            guides={[{ y: 16.7, label: "60 FPS" }, { y: 8.3, label: "120 FPS" }]}
          />
        )}
        {history.length > 0 && (
          <table>
            <thead>
              <tr><th></th><th>{t("Quando")}</th><th>{t("Jogo")}</th><th>{t("Tipo")}</th><th>FPS</th><th>1% low</th><th>{t("Ajustes")}</th></tr>
            </thead>
            <tbody>
              {history.slice(0, 30).map((h) => (
                <tr key={h.id}>
                  <td>
                    <input
                      type="checkbox"
                      checked={picked.includes(h.id)}
                      onChange={() => setPicked((p) => (p.includes(h.id) ? p.filter((x) => x !== h.id) : p.length < 4 ? [...p, h.id] : p))}
                    />
                  </td>
                  <td>{new Date(h.ts * 1000).toLocaleString(settings.lang)}</td>
                  <td>{h.game}</td>
                  <td>{labelName[h.label] ?? h.label}</td>
                  <td>{h.result.avg_fps.toFixed(1)}</td>
                  <td>{h.result.low_1pct_fps.toFixed(1)}</td>
                  <td>{h.tweaks.length}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {history.length > 0 && <p className="muted">{t("Marque até 4 medições para comparar no gráfico ou gerar um relatório.")}</p>}
      </section>
    </>
  );
}
