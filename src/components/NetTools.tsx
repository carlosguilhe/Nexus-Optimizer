import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Pause, Play, Route } from "lucide-react";
import { errText } from "../types";
import { useT } from "../i18n";
import { useToast } from "../toast";
import { BtnLabel, Loading, ProgressBar } from "./ui";
import { CHART_COLORS, LineChart } from "./charts";

interface Server {
  id: string;
  name: string;
  group: string;
  host: string;
}

interface ServerPing {
  id: string;
  ms: number | null;
  loss: number;
}

const msClass = (ms: number | null) => (ms == null ? "bad" : ms <= 40 ? "good" : ms <= 120 ? "mid" : "bad");

export function ServersCard() {
  const t = useT();
  const [servers, setServers] = useState<Server[]>([]);
  const [res, setRes] = useState<Record<string, ServerPing>>({});
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<Server[]>("server_list").then(setServers);
  }, []);

  async function run() {
    setBusy(true);
    try {
      const list = await invoke<ServerPing[]>("ping_servers", { rounds: 3 });
      setRes(Object.fromEntries(list.map((p) => [p.id, p])));
    } catch {
      /* sem rede: a tabela só fica vazia */
    }
    setBusy(false);
  }

  const groups = Array.from(new Set(servers.map((s) => s.group)));
  return (
    <section className="card">
      <h3>{t("Latência até regiões e serviços")}</h3>
      <p className="muted">
        {t("Mede a conexão até regiões da nuvem onde muitos jogos hospedam servidores e até os serviços das principais plataformas. Não é o servidor da sua partida, mas mostra a rota e a distância.")}
      </p>
      <div className="row">
        <button disabled={busy} onClick={run}>
          <BtnLabel busy={busy}>{busy ? t("Medindo…") : t("Medir agora")}</BtnLabel>
        </button>
      </div>
      {busy && <ProgressBar label={t("Medindo {n} destinos…", { n: servers.length })} />}
      {groups.map((g) => (
        <div key={g}>
          <h4 className="subhead">{t(g)}</h4>
          <ul className="bars">
            {servers
              .filter((s) => s.group === g)
              .sort((a, b) => (res[a.id]?.ms ?? 1e9) - (res[b.id]?.ms ?? 1e9))
              .map((s) => {
                const p = res[s.id];
                return (
                  <li key={s.id}>
                    <span>{t(s.name)}</span>
                    <div className="bar">
                      <i className={msClass(p?.ms ?? null)} style={{ width: p?.ms != null ? `${Math.min(100, (p.ms / 300) * 100)}%` : "0%" }} />
                    </div>
                    <b>{p ? (p.ms != null ? `${Math.round(p.ms)} ms` : t("sem resposta")) : "—"}</b>
                  </li>
                );
              })}
          </ul>
        </div>
      ))}
    </section>
  );
}

export function LivePing() {
  const t = useT();
  const [servers, setServers] = useState<Server[]>([]);
  const [host, setHost] = useState("br.api.riotgames.com");
  const [running, setRunning] = useState(false);
  const [samples, setSamples] = useState<(number | null)[]>([]);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    invoke<Server[]>("server_list").then(setServers);
    return () => window.clearInterval(timer.current);
  }, []);

  const stop = useCallback(() => {
    window.clearInterval(timer.current);
    setRunning(false);
  }, []);

  function start() {
    setSamples([]);
    setRunning(true);
    const tick = async () => {
      const ms = await invoke<number | null>("tcp_ping", { host, port: 443 });
      setSamples((s) => [...s.slice(-59), ms]);
    };
    tick();
    timer.current = window.setInterval(tick, 1000);
  }

  const ok = samples.filter((s): s is number => s != null);
  const avg = ok.length ? ok.reduce((a, b) => a + b, 0) / ok.length : 0;
  const jitter = ok.length > 1 ? ok.slice(1).reduce((a, b, i) => a + Math.abs(b - ok[i]), 0) / (ok.length - 1) : 0;
  const loss = samples.length ? ((samples.length - ok.length) / samples.length) * 100 : 0;
  const max = Math.max(60, Math.ceil(Math.max(...ok, 0) / 20) * 20);

  return (
    <section className="card">
      <h3>{t("Ping ao vivo")}</h3>
      <p className="muted">{t("Acompanhe picos de latência e perda de pacotes enquanto joga. Uma medição por segundo.")}</p>
      <div className="row">
        <select value={host} onChange={(e) => setHost(e.currentTarget.value)} disabled={running}>
          {servers.map((s) => (
            <option key={s.id} value={s.host}>
              {t(s.name)}
            </option>
          ))}
        </select>
        {running ? (
          <button className="ghost" onClick={stop}>
            <Pause size={15} /> {t("Parar")}
          </button>
        ) : (
          <button onClick={start}>
            <Play size={15} /> {t("Iniciar")}
          </button>
        )}
      </div>
      <LineChart series={[{ values: samples.map((s) => s ?? max), color: CHART_COLORS[0] }]} max={max} guides={[{ y: 50, label: "50 ms" }]} />
      <div className="stats">
        <div><span>{t("Média")}</span><b>{ok.length ? `${avg.toFixed(0)} ms` : "—"}</b></div>
        <div><span>Mín / Máx</span><b>{ok.length ? `${Math.min(...ok).toFixed(0)} / ${Math.max(...ok).toFixed(0)} ms` : "—"}</b></div>
        <div><span>Jitter</span><b>{ok.length > 1 ? `${jitter.toFixed(1)} ms` : "—"}</b></div>
        <div><span>{t("Perda")}</span><b>{samples.length ? `${loss.toFixed(0)}%` : "—"}</b></div>
      </div>
    </section>
  );
}

interface Dns {
  name: string;
  ip: string;
  preset: string | null;
  ms: number | null;
  failures: number;
}

export function DnsBench({ admin }: { admin: boolean }) {
  const t = useT();
  const toast = useToast();
  const [rows, setRows] = useState<Dns[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);

  async function run() {
    setBusy(true);
    try {
      setRows(await invoke<Dns[]>("dns_benchmark"));
    } catch (e) {
      toast(t(errText(e)));
    }
    setBusy(false);
  }

  async function use(r: Dns) {
    if (!r.preset) return;
    setApplying(r.ip);
    try {
      await invoke("set_dns", { preset: r.preset });
      toast(t("DNS alterado para {n}.", { n: r.name }));
    } catch (e) {
      toast(t(errText(e)));
    }
    setApplying(null);
  }

  return (
    <section className="card">
      <h3>{t("Qual DNS é mais rápido para você")}</h3>
      <p className="muted">{t("Testa cada servidor DNS com consultas reais a partir deste PC. DNS rápido acelera a abertura de sites; não reduz o ping dentro do jogo.")}</p>
      <div className="row">
        <button disabled={busy} onClick={run}>
          <BtnLabel busy={busy}>{busy ? t("Testando…") : t("Testar DNS")}</BtnLabel>
        </button>
      </div>
      {busy && <ProgressBar label={t("Consultando 5 servidores…")} />}
      {rows && (
        <table>
          <thead>
            <tr><th>{t("Servidor")}</th><th>IP</th><th>{t("Resposta")}</th><th></th></tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={r.ip}>
                <td>{r.name === "Atual do sistema" ? t(r.name) : r.name} {i === 0 && r.ms != null && <span className="chip cat">{t("mais rápido")}</span>}</td>
                <td>{r.ip}</td>
                <td>{r.ms != null ? `${r.ms.toFixed(1)} ms` : t("sem resposta")}{r.failures > 0 && r.ms != null ? ` (${r.failures} ${t("falhas")})` : ""}</td>
                <td className="right">
                  {r.preset && r.ms != null && (
                    <button className="ghost small" disabled={!admin || applying !== null} onClick={() => use(r)}>
                      <BtnLabel busy={applying === r.ip}>{t("Usar")}</BtnLabel>
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {rows && !admin && <p className="muted">{t("Para trocar o DNS, abra o app como administrador.")}</p>}
    </section>
  );
}

interface Hop {
  hop: number;
  ms: number | null;
  ip: string | null;
}

export function RouteTrace() {
  const t = useT();
  const [host, setHost] = useState("br.api.riotgames.com");
  const [hops, setHops] = useState<Hop[]>([]);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  useEffect(() => {
    const un = listen<Hop>("trace", (e) => setHops((h) => [...h, e.payload]));
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function run() {
    setBusy(true);
    setErr("");
    setHops([]);
    try {
      await invoke("route_trace", { host: host.trim() });
    } catch (e) {
      setErr(t(errText(e)));
    }
    setBusy(false);
  }

  return (
    <section className="card">
      <h3>{t("Rota até o servidor")}</h3>
      <p className="muted">{t("Mostra cada salto da sua conexão até o destino. Onde o tempo sobe de repente costuma estar o problema (seu roteador, o provedor ou o destino).")}</p>
      <div className="row">
        <input value={host} onChange={(e) => setHost(e.currentTarget.value)} placeholder="br.api.riotgames.com" />
        <button disabled={busy || !host.trim()} onClick={run}>
          <BtnLabel busy={busy}>
            <Route size={15} /> {busy ? t("Traçando…") : t("Traçar rota")}
          </BtnLabel>
        </button>
      </div>
      {busy && <ProgressBar label={t("Salto {n}…", { n: hops.length + 1 })} />}
      {err && <p className="error">{err}</p>}
      {hops.length > 0 && (
        <table>
          <thead>
            <tr><th>#</th><th>IP</th><th>{t("Tempo")}</th><th></th></tr>
          </thead>
          <tbody>
            {hops.map((h, i) => {
              const prev = hops.slice(0, i).reverse().find((x) => x.ms != null)?.ms ?? 0;
              const jump = h.ms != null && h.ms - prev > 50;
              return (
                <tr key={h.hop}>
                  <td>{h.hop}</td>
                  <td>{h.ip ?? "*"}</td>
                  <td>{h.ms != null ? `${h.ms.toFixed(0)} ms` : t("sem resposta")}</td>
                  <td>{jump && <span className="chip risk-moderado">{t("salto grande")}</span>}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      {!busy && hops.length === 0 && !err && <Loading text={t("Pronto para traçar.")} />}
    </section>
  );
}
