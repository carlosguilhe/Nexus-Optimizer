import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, CircleCheck, CircleX, ExternalLink, FolderOpen, ScanSearch, Square } from "lucide-react";
import { errText, fmtBytes, Tweak } from "../types";
import { FeaturesCard, WindowsUpdateCard } from "../components/WinSystem";
import { useT } from "../i18n";
import { BtnLabel, Loading, PageHeader, ProgressBar } from "../components/ui";
import { useToast } from "../toast";
import { tabIcon } from "../icons";

/* eslint-disable @typescript-eslint/no-explicit-any */
const arr = (v: any): any[] => (v == null ? [] : Array.isArray(v) ? v : [v]);

function DiskHealth() {
  const t = useT();
  const [data, setData] = useState<any>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    invoke("disk_health").then(setData).catch((e) => setErr(errText(e)));
  }, []);

  const trimOn = data && /=\s*0/.test(String(data.trim ?? ""));
  return (
    <section className="card">
      <h3>{t("Saúde dos discos")}</h3>
      {!data && !err && <Loading text={t("Lendo os discos…")} />}
      {err && <p className="error">{t(err)}</p>}
      {data && (
        <>
          <ul className="disks">
            {arr(data.disks).map((d, i) => {
              const ok = d.HealthStatus === "Healthy";
              return (
                <li key={i}>
                  {ok ? <CircleCheck size={18} className="ok-text" /> : <CircleX size={18} className="accent" />}
                  <div>
                    <b>{d.FriendlyName}</b>
                    <small>
                      {d.MediaType} · {fmtBytes(d.Size)} · {ok ? t("Saudável") : String(d.HealthStatus)}
                    </small>
                  </div>
                </li>
              );
            })}
          </ul>
          {arr(data.volumes).map((v, i) => {
            const used = v.Size ? ((v.Size - v.SizeRemaining) / v.Size) * 100 : 0;
            return (
              <div key={i} className="vol">
                <span>
                  {v.DriveLetter}: {v.FileSystemLabel}
                </span>
                <div className="bar"><i className={used > 90 ? "bad" : used > 75 ? "mid" : ""} style={{ width: `${used}%` }} /></div>
                <b>{t("{free} livres de {total}", { free: fmtBytes(v.SizeRemaining), total: fmtBytes(v.Size) })}</b>
              </div>
            );
          })}
          <p className="muted">
            TRIM: {trimOn ? t("ativado (bom para SSD)") : t("verifique: pode estar desativado")}
          </p>
        </>
      )}
    </section>
  );
}

function WinCheck({ admin }: { admin: boolean }) {
  const t = useT();
  const toast = useToast();
  const [busy, setBusy] = useState<string | null>(null);
  const [prog, setProg] = useState<{ task: string; progress: number | null; line: string } | null>(null);
  const [res, setRes] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    const un = listen<{ task: string; progress: number | null; line: string }>("maint", (e) => setProg(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function run(task: "sfc" | "dism") {
    setBusy(task);
    setRes(null);
    setProg(null);
    try {
      const text = await invoke<string>("maint_run", { task });
      setRes({ ok: true, text });
      toast(t("Verificação concluída."));
    } catch (e) {
      setRes({ ok: false, text: t(errText(e)) });
    }
    setBusy(null);
  }

  return (
    <section className="card">
      <h3>{t("Integridade do Windows")}</h3>
      <p className="muted">
        {t("SFC verifica e repara arquivos do sistema; DISM repara a imagem do Windows. Levam vários minutos e exigem administrador. Use se o Windows estiver instável ou com erros estranhos.")}
      </p>
      <div className="row">
        <button disabled={!admin || busy !== null} onClick={() => run("sfc")}>
          <BtnLabel busy={busy === "sfc"}>{t("Verificar arquivos (SFC)")}</BtnLabel>
        </button>
        <button className="ghost" disabled={!admin || busy !== null} onClick={() => run("dism")}>
          <BtnLabel busy={busy === "dism"}>{t("Reparar imagem (DISM)")}</BtnLabel>
        </button>
      </div>
      {!admin && <p className="warn">{t("Exige administrador.")}</p>}
      {busy && (
        <ProgressBar
          value={prog?.progress != null ? prog.progress / 100 : undefined}
          label={prog?.progress != null ? `${prog.progress.toFixed(0)}%` : t("Iniciando… isso pode levar vários minutos")}
        />
      )}
      {res && <p className={res.ok ? "ok" : "error"}>{res.text || t("Concluído.")}</p>}
    </section>
  );
}

interface Dir {
  name: string;
  path: string;
  bytes: number;
  files: number;
  is_dir: boolean;
}

function DiskAnalyzer() {
  const t = useT();
  const [roots, setRoots] = useState<[string, string][]>([]);
  const [path, setPath] = useState("");
  const [stack, setStack] = useState<string[]>([]);
  const [items, setItems] = useState<Dir[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [prog, setProg] = useState({ current: "", done: 0, total: 1 });
  const [err, setErr] = useState("");
  const seq = useRef(0);

  useEffect(() => {
    invoke<[string, string][]>("disk_roots").then((r) => {
      setRoots(r);
      if (r[0]) setPath(r[0][1]);
    });
    const un = listen<{ current: string; done: number; total: number }>("diskscan", (e) => setProg(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function scan(target: string) {
    const my = ++seq.current;
    setBusy(true);
    setErr("");
    try {
      const r = await invoke<Dir[]>("disk_scan", { root: target });
      if (my === seq.current) {
        setItems(r);
        setPath(target);
      }
    } catch (e) {
      if (my === seq.current) setErr(t(errText(e)));
    }
    if (my === seq.current) setBusy(false);
  }

  const max = items?.[0]?.bytes || 1;
  return (
    <section className="card">
      <h3>{t("Analisador de espaço em disco")}</h3>
      <p className="muted">{t("Mostra quais pastas ocupam mais espaço. Clique numa pasta para ver o que há dentro. Só lê; não apaga nada.")}</p>
      <div className="row">
        <select
          value={stack.length ? "" : path}
          onChange={(e) => {
            setStack([]);
            setPath(e.currentTarget.value);
            setItems(null);
          }}
          disabled={busy}
        >
          {stack.length > 0 && <option value="">{path}</option>}
          {roots.map(([label, p]) => (
            <option key={p} value={p}>
              {t(label.startsWith("Unidade") ? label.replace("Unidade", t("Unidade")) : label)}
            </option>
          ))}
        </select>
        {busy ? (
          <button className="ghost" onClick={() => invoke("disk_scan_cancel")}>
            <Square size={14} /> {t("Cancelar")}
          </button>
        ) : (
          <button onClick={() => scan(path)}>
            <ScanSearch size={15} /> {t("Analisar")}
          </button>
        )}
        {stack.length > 0 && !busy && (
          <button
            className="ghost"
            onClick={() => {
              const prev = stack[stack.length - 1];
              setStack(stack.slice(0, -1));
              scan(prev);
            }}
          >
            <ArrowLeft size={15} /> {t("Voltar")}
          </button>
        )}
      </div>
      {busy && <ProgressBar value={prog.done / Math.max(1, prog.total)} label={`${prog.current} (${prog.done}/${prog.total})`} />}
      {err && <p className="error">{err}</p>}
      {items && !busy && (
        <ul className="bars wide">
          {items.map((d, i) => (
            <li key={i}>
              {d.is_dir ? (
                <button className="link" onClick={() => { setStack([...stack, path]); scan(d.path); }}>
                  <FolderOpen size={14} /> {d.name}
                </button>
              ) : (
                <span className="muted">{t("(arquivos soltos)")}</span>
              )}
              <div className="bar"><i style={{ width: `${(d.bytes / max) * 100}%` }} /></div>
              <b>{fmtBytes(d.bytes)}</b>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function Drivers() {
  const t = useT();
  const [list, setList] = useState<any[] | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    invoke("drivers").then((d) => setList(arr(d))).catch((e) => setErr(errText(e)));
  }, []);

  const vendor = (m: string) => {
    const s = m.toLowerCase();
    if (s.includes("nvidia")) return "nvidia";
    if (s.includes("advanced micro") || s.includes("amd")) return "amd";
    if (s.includes("intel")) return "intel";
    if (s.includes("realtek")) return "realtek";
    return null;
  };
  const age = (d: string) => (d ? Math.floor((Date.now() - new Date(d).getTime()) / (365.25 * 86400000)) : null);

  return (
    <section className="card">
      <h3>{t("Drivers de vídeo, rede e áudio")}</h3>
      <p className="muted">{t("Mostra a versão e a data de cada driver e leva ao site oficial do fabricante. O Nexus não baixa nem instala drivers sozinho.")}</p>
      {!list && !err && <Loading text={t("Lendo drivers…")} />}
      {err && <p className="error">{t(err)}</p>}
      <ul className="drivers">
        {list?.map((d, i) => {
          const v = vendor(String(d.Manufacturer ?? ""));
          const years = age(d.DriverDate);
          return (
            <li key={i}>
              <div>
                <b>{d.DeviceName}</b>
                <small>
                  {d.DeviceClass} · v{d.DriverVersion} · {d.DriverDate || "—"}
                  {years != null && years >= 2 && <span className="chip risk-moderado">{t("{n} anos", { n: years })}</span>}
                </small>
              </div>
              {v && (
                <button className="ghost small" onClick={() => invoke("open_target", { key: v })}>
                  <ExternalLink size={13} /> {t("Site oficial")}
                </button>
              )}
            </li>
          );
        })}
      </ul>
      <div className="row">
        <button className="ghost" onClick={() => invoke("open_target", { key: "winupdate" })}>
          <ExternalLink size={14} /> Windows Update
        </button>
        <button className="ghost" onClick={() => invoke("open_target", { key: "display" })}>
          <ExternalLink size={14} /> {t("Configurações de tela")}
        </button>
      </div>
    </section>
  );
}

export function MaintTab({ admin, updateTweaks, refresh }: { admin: boolean; updateTweaks: Tweak[]; refresh: () => Promise<void> }) {
  const t = useT();
  return (
    <>
      <PageHeader icon={tabIcon("Manutenção")} title={t("Manutenção")} desc={t("Saúde dos discos, reparo do Windows, atualizações, recursos, espaço em disco e drivers.")} />
      <DiskHealth />
      <WinCheck admin={admin} />
      <WindowsUpdateCard tweaks={updateTweaks} admin={admin} refresh={refresh} />
      <FeaturesCard admin={admin} />
      <DiskAnalyzer />
      <Drivers />
    </>
  );
}
