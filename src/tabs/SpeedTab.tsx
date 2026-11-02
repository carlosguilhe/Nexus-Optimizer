import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Gauge } from "../components/Gauge";
import { BtnLabel, PageHeader, Spinner } from "../components/ui";
import { errText } from "../types";
import { useT, T } from "../i18n";
import { useSettings } from "../settings";
import { tabIcon } from "../icons";

interface Progress {
  phase: "ping" | "download" | "upload" | "done";
  value: number;
  progress: number;
}

interface Result {
  ping_ms: number;
  jitter_ms: number;
  down_mbps: number;
  up_mbps: number;
  server: string;
}

interface Saved extends Result {
  at: string;
}

const HISTORY_KEY = "nexus.speed.history";

function loadHistory(): Saved[] {
  try {
    return JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]");
  } catch {
    return [];
  }
}

function verdict(r: Result, t: T): { text: string; level: string } {
  if (r.ping_ms <= 25 && r.jitter_ms <= 8) {
    return { text: t("Ótima para jogos online competitivos: latência baixa e estável."), level: "good" };
  }
  if (r.ping_ms <= 60 && r.jitter_ms <= 20) {
    return { text: t("Boa para a maioria dos jogos. Picos de jitter podem causar pequenas oscilações."), level: "ok" };
  }
  return {
    text: t("Latência ou jitter altos: pode haver lag. Prefira cabo de rede, feche downloads e teste o servidor do seu jogo na aba Rede."),
    level: "bad",
  };
}

export function SpeedTab() {
  const t = useT();
  const { settings } = useSettings();
  const [running, setRunning] = useState(false);
  const [phase, setPhase] = useState<Progress["phase"] | "idle">("idle");
  const [live, setLive] = useState(0);
  const [progress, setProgress] = useState(0);
  const [result, setResult] = useState<Result | null>(null);
  const [partial, setPartial] = useState<Partial<Result>>({});
  const [err, setErr] = useState("");
  const [history, setHistory] = useState<Saved[]>(loadHistory);
  const live2 = useRef<Partial<Result>>({});

  useEffect(() => {
    const un = listen<Progress>("speedtest", (e) => {
      const p = e.payload;
      setPhase(p.phase);
      setProgress(p.progress);
      if (p.phase === "ping") {
        live2.current = { ...live2.current, ping_ms: p.value };
        setPartial({ ...live2.current });
        setLive(0);
      } else if (p.phase === "download" || p.phase === "upload") {
        setLive(p.value);
        const key = p.phase === "download" ? "down_mbps" : "up_mbps";
        live2.current = { ...live2.current, [key]: p.value };
        setPartial({ ...live2.current });
      } else {
        setLive(0);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function start() {
    setRunning(true);
    setErr("");
    setResult(null);
    live2.current = {};
    setPartial({});
    setLive(0);
    try {
      const r = await invoke<Result>("speed_test");
      setResult(r);
      setPartial(r);
      const next = [{ ...r, at: new Date().toLocaleString(settings.lang) }, ...history].slice(0, 8);
      setHistory(next);
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
      } catch {
        /* armazenamento indisponível: o histórico só não persiste */
      }
    } catch (e) {
      setErr(t(errText(e)));
    }
    setRunning(false);
    setPhase("idle");
    setLive(0);
  }

  const label =
    phase === "ping" ? t("latência") : phase === "upload" ? "upload" : phase === "download" || result ? "download" : t("pronto");
  // Depois do teste, o velocímetro fica no resultado de download.
  const shown = result && !running ? result.down_mbps : live;
  const v = result ? verdict(result, t) : null;

  return (
    <>
      <PageHeader
        icon={tabIcon("Velocidade")}
        title={t("Velocidade")}
        desc={t("Mede latência (conexão TCP, próxima do ping dos jogos), jitter, download e upload. Se o servidor principal limitar as requisições, o teste usa um servidor reserva automaticamente. Dura cerca de 25 segundos e pode transferir algumas centenas de MB.")}
      />

      <section className="card speed">
        <Gauge value={shown} label={label} />
        <div className="progressbar">
          <div style={{ width: `${running ? progress * 100 : 0}%` }} />
        </div>
        <p className="muted center status-line">
          {running && <Spinner size={14} />}
          {running
            ? phase === "ping"
              ? t("Medindo latência…")
              : phase === "download"
                ? t("Medindo download…")
                : phase === "upload"
                  ? t("Medindo upload…")
                  : t("Finalizando…")
            : result
              ? t("Teste concluído.")
              : t("Pronto para testar.")}
        </p>
        <div className="center">
          <button disabled={running} onClick={start}>
            <BtnLabel busy={running}>{running ? t("Testando…") : result ? t("Testar novamente") : t("Iniciar teste")}</BtnLabel>
          </button>
        </div>
        {err && <p className="error">{err}</p>}

        <div className="stats">
          <div><span>Ping</span><b>{partial.ping_ms != null ? `${partial.ping_ms.toFixed(0)} ms` : "—"}</b></div>
          <div><span>Jitter</span><b>{result ? `${result.jitter_ms.toFixed(1)} ms` : "—"}</b></div>
          <div><span>Download</span><b>{partial.down_mbps != null ? `${partial.down_mbps.toFixed(1)} Mbps` : "—"}</b></div>
          <div><span>Upload</span><b>{partial.up_mbps != null ? `${partial.up_mbps.toFixed(1)} Mbps` : "—"}</b></div>
        </div>
        {result && <p className="muted center">{t("Servidor de download: {s}", { s: result.server })}</p>}
        {v && <p className={`verdict ${v.level}`}>{v.text}</p>}
      </section>

      {history.length > 0 && (
        <section className="card">
          <h3>{t("Histórico")}</h3>
          <table>
            <thead>
              <tr><th>{t("Quando")}</th><th>Ping</th><th>Jitter</th><th>Download</th><th>Upload</th></tr>
            </thead>
            <tbody>
              {history.map((h) => (
                <tr key={h.at}>
                  <td>{h.at}</td>
                  <td>{h.ping_ms.toFixed(0)} ms</td>
                  <td>{h.jitter_ms.toFixed(1)} ms</td>
                  <td>{h.down_mbps.toFixed(1)} Mbps</td>
                  <td>{h.up_mbps.toFixed(1)} Mbps</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </>
  );
}
