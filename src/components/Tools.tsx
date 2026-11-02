import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errText, fmtBytes } from "../types";
import { useToast } from "../toast";
import { useConfirm } from "../confirm";
import { useT } from "../i18n";
import { BtnLabel, Loading, ProgressBar, Switch } from "./ui";

interface Ping {
  host: string;
  sent: number;
  received: number;
  loss_pct: number;
  min: number;
  avg: number;
  max: number;
  jitter: number;
}

export function PingTool() {
  const t = useT();
  const [host, setHost] = useState("1.1.1.1");
  const [count, setCount] = useState(20);
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<Ping | null>(null);
  const [err, setErr] = useState("");

  async function run() {
    setBusy(true);
    setErr("");
    try {
      setRes(await invoke<Ping>("ping_test", { host, count }));
    } catch (e) {
      setErr(t(errText(e)));
      setRes(null);
    }
    setBusy(false);
  }

  return (
    <section className="card">
      <h3>{t("Teste de latência")}</h3>
      <p className="muted">
        {t("Mede ping, jitter e perda de pacotes até um servidor. Use o IP/endereço do servidor do seu jogo para um resultado real; DNS e ajustes de TCP não baixam o ping, a rota é que decide.")}
      </p>
      <div className="row">
        <input value={host} onChange={(e) => setHost(e.currentTarget.value)} placeholder="1.1.1.1" />
        <input type="number" min={4} max={100} value={count} onChange={(e) => setCount(Number(e.currentTarget.value))} />
        <button disabled={busy || !host} onClick={run}>
          <BtnLabel busy={busy}>{busy ? t("Medindo…") : t("Testar")}</BtnLabel>
        </button>
      </div>
      {err && <p className="error">{err}</p>}
      {res && (
        <div className="stats">
          <div><span>{t("Média")}</span><b>{res.avg.toFixed(1)} ms</b></div>
          <div><span>Mín / Máx</span><b>{res.min.toFixed(0)} / {res.max.toFixed(0)} ms</b></div>
          <div><span>Jitter</span><b>{res.jitter.toFixed(1)} ms</b></div>
          <div><span>{t("Perda")}</span><b>{res.loss_pct.toFixed(0)}%</b></div>
        </div>
      )}
    </section>
  );
}

export function PriorityTool() {
  const t = useT();
  const [proc, setProc] = useState("");
  const [level, setLevel] = useState("High");
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");

  async function run() {
    setMsg("");
    setErr("");
    try {
      const n = await invoke<number>("set_priority", { process: proc, level });
      setMsg(t("Prioridade {level} aplicada em {n} processo(s). Volta ao normal quando o jogo fechar.", { level, n }));
    } catch (e) {
      setErr(t(errText(e)));
    }
  }

  return (
    <section className="card">
      <h3>{t("Prioridade do jogo")}</h3>
      <p className="muted">
        {t("Com o jogo aberto, eleva a prioridade de CPU dele. Não é permanente. Se o jogo rodar como administrador, abra o Nexus também como administrador.")}
      </p>
      <div className="row">
        <input value={proc} onChange={(e) => setProc(e.currentTarget.value)} placeholder="ex.: VALORANT-Win64-Shipping.exe" />
        <select value={level} onChange={(e) => setLevel(e.currentTarget.value)}>
          <option value="High">{t("Alta")}</option>
          <option value="AboveNormal">{t("Acima do normal")}</option>
          <option value="Normal">Normal</option>
        </select>
        <button disabled={!proc} onClick={run}>{t("Aplicar")}</button>
      </div>
      {msg && <p className="ok">{msg}</p>}
      {err && <p className="error">{err}</p>}
    </section>
  );
}

interface Cat {
  id: string;
  name: string;
  description: string;
  admin: boolean;
}

interface Rep {
  id: string;
  files: number;
  bytes: number;
}

export function CleanTool({ admin }: { admin: boolean }) {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const [cats, setCats] = useState<Cat[]>([]);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [reps, setReps] = useState<Record<string, Rep>>({});
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  useEffect(() => {
    invoke<Cat[]>("clean_categories").then((c) => {
      setCats(c);
      // Pré-seleciona o que é comum; a Lixeira (irreversível) fica desmarcada.
      setSel(new Set(c.filter((x) => x.id !== "recycle" && (!x.admin || admin)).map((x) => x.id)));
    });
  }, [admin]);

  const ids = [...sel];
  const total = ids.reduce((s, id) => s + (reps[id]?.bytes ?? 0), 0);
  const scanned = ids.some((id) => reps[id]);

  function toggle(id: string) {
    setSel((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  }

  async function scan() {
    setBusy(true);
    setErr("");
    try {
      const list = await invoke<Rep[]>("clean_scan", { ids });
      setReps((r) => ({ ...r, ...Object.fromEntries(list.map((x) => [x.id, x])) }));
    } catch (e) {
      setErr(t(errText(e)));
    }
    setBusy(false);
  }

  async function clean() {
    const ok = await confirm({
      title: t("Apagar {size}?", { size: fmtBytes(total) }),
      message: (
        <>
          {t("{n} categoria(s) serão limpas.", { n: ids.length })}
          {sel.has("recycle") && <> {t("A Lixeira será esvaziada.")}</>} {t("Os arquivos apagados não podem ser recuperados.")}
        </>
      ),
      confirmLabel: t("Apagar"),
      danger: true,
    });
    if (!ok) return;
    setBusy(true);
    setErr("");
    try {
      const done = await invoke<Rep[]>("clean_run", { ids });
      const freed = done.reduce((s, r) => s + r.bytes, 0);
      setReps((r) => {
        const n = { ...r };
        done.forEach((d) => delete n[d.id]);
        return n;
      });
      toast(t("Limpeza concluída: {size} liberados.", { size: fmtBytes(freed) }));
    } catch (e) {
      setErr(t(errText(e)));
    }
    setBusy(false);
  }

  return (
    <section className="card">
      <h3>{t("Limpeza")}</h3>
      <p className="muted">
        {t("Libera espaço em disco; não aumenta FPS. O cache de shaders dos jogos não é apagado de propósito, porque apagá-lo causa travadas até ser recriado. Clique em Analisar para ver o tamanho antes de limpar.")}
      </p>
      <ul className="checks">
        {cats.map((c) => {
          const disabled = c.admin && !admin;
          return (
            <li key={c.id} className={disabled ? "off" : ""}>
              <label>
                <input type="checkbox" checked={sel.has(c.id)} disabled={disabled} onChange={() => toggle(c.id)} />
                <span>
                  <b>{t(c.name)}</b>
                  {c.admin && <span className="chip">admin</span>}
                  <small>{t(c.description)}</small>
                </span>
              </label>
              <em>{reps[c.id] ? `${fmtBytes(reps[c.id].bytes)} · ${reps[c.id].files} ${t("arq.")}` : ""}</em>
            </li>
          );
        })}
      </ul>
      <div className="row">
        <button className="ghost" disabled={busy || ids.length === 0} onClick={scan}>
          <BtnLabel busy={busy}>{busy ? t("Aguarde…") : t("Analisar")}</BtnLabel>
        </button>
        <button disabled={busy || !scanned || total === 0} onClick={clean}>
          {t("Limpar")} {scanned && total > 0 ? fmtBytes(total) : ""}
        </button>
      </div>
      {busy && <ProgressBar label={scanned ? t("Limpando… isso pode levar alguns segundos") : t("Analisando as pastas…")} />}
      {err && <p className="error">{err}</p>}
    </section>
  );
}

export function MemoryTool() {
  const t = useT();
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<[number, number] | null>(null);

  async function run() {
    setBusy(true);
    try {
      const r = await invoke<[number, number]>("free_memory");
      setRes(r);
      toast(t("Memória: {size} liberados agora.", { size: fmtBytes(Math.max(0, r[0] - r[1])) }));
    } catch (e) {
      toast(t(errText(e)));
    }
    setBusy(false);
  }

  return (
    <section className="card">
      <h3>{t("Liberar memória")}</h3>
      <p className="muted">
        {t("Pede ao Windows para devolver a RAM que os programas deixaram parada. O efeito é temporário: o que o programa voltar a usar é carregado de novo. Jogos detectados não são afetados.")}
      </p>
      <div className="row">
        <button disabled={busy} onClick={run}>
          <BtnLabel busy={busy}>{busy ? t("Liberando…") : t("Liberar memória")}</BtnLabel>
        </button>
        {res && <small className="muted">{t("{a} → {b} em uso", { a: fmtBytes(res[0]), b: fmtBytes(res[1]) })}</small>}
      </div>
    </section>
  );
}

export interface GameMode {
  enabled: boolean;
  active: boolean;
  game: string | null;
}

export function GameModeCard({ gm, onChange }: { gm: GameMode; onChange: (on: boolean) => void }) {
  const t = useT();
  return (
    <section className={`card gm ${gm.active ? "live" : ""}`}>
      <div className="row spread">
        <div>
          <h3>{t("Modo Jogo automático")}</h3>
          <p className="muted">
            {t("Quando um jogo do catálogo abrir, o Nexus aplica o perfil dele (prioridade, ajustes e programas a fechar) e ativa o plano de energia de alto desempenho; ao fechar, desfaz tudo. Fica ativo enquanto o app estiver aberto ou na bandeja.")}
          </p>
        </div>
        <Switch checked={gm.enabled} onChange={onChange} />
      </div>
      {gm.active && <p className="ok">{t("Ativo agora em:")} <b>{gm.game}</b></p>}
      {gm.enabled && !gm.active && <p className="muted">{t("Aguardando um jogo abrir…")}</p>}
    </section>
  );
}

export function DnsTool({ admin }: { admin: boolean }) {
  const t = useT();
  const [current, setCurrent] = useState<string | null>(null);
  const [preset, setPreset] = useState("cloudflare");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");

  async function load() {
    try {
      setCurrent((await invoke<string>("get_dns")) || t("automático"));
    } catch (e) {
      setCurrent(t("indisponível"));
      setErr(t(errText(e)));
    }
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function apply(p: string) {
    setBusy(true);
    setErr("");
    setMsg("");
    try {
      await invoke("set_dns", { preset: p });
      setMsg(p === "auto" ? t("DNS voltou ao automático (roteador).") : t("DNS alterado."));
    } catch (e) {
      setErr(t(errText(e)));
    }
    await load();
    setBusy(false);
  }

  return (
    <section className="card">
      <h3>{t("Servidor DNS")}</h3>
      <p className="muted">
        {t("DNS mais rápido acelera a abertura de sites e a conexão inicial. Não reduz o ping dentro do jogo. Anote o DNS atual antes de trocar: \"Automático\" volta ao padrão do roteador, não a um DNS manual que você já tenha usado.")}
      </p>
      <p>
        {t("Atual:")} {current === null ? <Loading text={t("Lendo DNS…")} /> : <b>{current}</b>}
      </p>
      <div className="row">
        <select value={preset} onChange={(e) => setPreset(e.currentTarget.value)}>
          <option value="cloudflare">Cloudflare (1.1.1.1)</option>
          <option value="google">Google (8.8.8.8)</option>
          <option value="quad9">Quad9 (9.9.9.9)</option>
          <option value="opendns">OpenDNS (208.67.222.222)</option>
        </select>
        <button disabled={busy || !admin} onClick={() => apply(preset)}>{t("Aplicar")}</button>
        <button className="ghost" disabled={busy || !admin} onClick={() => apply("auto")}>{t("Automático")}</button>
      </div>
      {!admin && <p className="warn">{t("Exige administrador.")}</p>}
      {busy && <ProgressBar label={t("Aplicando o DNS nos adaptadores de rede…")} />}
      {msg && <p className="ok">{msg}</p>}
      {err && <p className="error">{err}</p>}
    </section>
  );
}

export function RestoreCard({ admin }: { admin: boolean }) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");

  async function create() {
    setBusy(true);
    setMsg("");
    setErr("");
    try {
      setMsg(t(await invoke<string>("restore_point")));
    } catch (e) {
      setErr(t(errText(e)));
    }
    setBusy(false);
  }

  return (
    <section className="card">
      <h3>{t("Ponto de restauração")}</h3>
      <p className="muted">
        {t("Um instantâneo do Windows para voltar atrás se algo der errado. Recomendado antes de aplicar ajustes avançados. Cada ajuste do Nexus também tem o próprio botão Reverter.")}
      </p>
      <div className="row">
        <button disabled={busy || !admin} onClick={create}>
          <BtnLabel busy={busy}>{busy ? t("Criando…") : t("Criar ponto de restauração")}</BtnLabel>
        </button>
        {!admin && <small className="muted">{t("Exige administrador.")}</small>}
        <button className="ghost small" onClick={() => invoke("open_target", { key: "sysprotect" })}>
          {t("Proteção do Sistema")}
        </button>
      </div>
      {busy && <ProgressBar label={t("Criando o ponto de restauração… pode levar até um minuto")} />}
      {msg && <p className="ok">{msg}</p>}
      {err && <p className="error">{err}</p>}
    </section>
  );
}
