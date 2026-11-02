import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errText, fmtBytes } from "../types";
import { useT } from "../i18n";
import { Loading, PageHeader } from "../components/ui";
import { Sparkline } from "../components/charts";
import { tabIcon } from "../icons";

interface Usage {
  cpu_pct: number;
  ram_used: number;
  ram_total: number;
}

interface Gpu {
  name: string;
  temp_c: number | null;
  util_pct: number | null;
  mem_used_mb: number | null;
  mem_total_mb: number | null;
  power_w: number | null;
  clock_mhz: number | null;
  fan_pct: number | null;
}

const KEEP = 90;
const push = (arr: number[], v: number) => [...arr.slice(-(KEEP - 1)), v];
const tempClass = (c: number | null) => (c == null ? "" : c >= 85 ? "bad" : c >= 75 ? "mid" : "good");

export function MonitorTab() {
  const t = useT();
  const [cpu, setCpu] = useState<number[]>([]);
  const [ram, setRam] = useState<number[]>([]);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [gpus, setGpus] = useState<Gpu[] | null>(null);
  const [gpuErr, setGpuErr] = useState("");
  const [gTemp, setGTemp] = useState<number[]>([]);
  const [gUtil, setGUtil] = useState<number[]>([]);

  useEffect(() => {
    let alive = true;
    const sample = async () => {
      const u = await invoke<Usage>("usage").catch(() => null);
      if (alive && u) {
        setUsage(u);
        setCpu((a) => push(a, u.cpu_pct));
        setRam((a) => push(a, u.ram_total ? (u.ram_used / u.ram_total) * 100 : 0));
      }
    };
    const sampleGpu = async () => {
      try {
        const g = await invoke<Gpu[]>("gpu_stats");
        if (!alive) return;
        setGpus(g);
        setGpuErr("");
        if (g[0]) {
          setGTemp((a) => push(a, g[0].temp_c ?? 0));
          setGUtil((a) => push(a, g[0].util_pct ?? 0));
        }
      } catch (e) {
        if (alive) {
          setGpus([]);
          setGpuErr(errText(e));
        }
      }
    };
    sample();
    sampleGpu();
    const a = setInterval(sample, 1000);
    const b = setInterval(sampleGpu, 2000);
    return () => {
      alive = false;
      clearInterval(a);
      clearInterval(b);
    };
  }, []);

  const ramPct = usage && usage.ram_total ? (usage.ram_used / usage.ram_total) * 100 : 0;
  const g = gpus?.[0];

  return (
    <>
      <PageHeader icon={tabIcon("Monitor")} title={t("Monitor")} desc={t("Uso do processador, memória e placa de vídeo em tempo real.")} />

      <div className="monitor-grid">
        <section className="card meter">
          <span>CPU</span>
          <b>{cpu.length ? `${cpu[cpu.length - 1].toFixed(0)}%` : "—"}</b>
          <Sparkline values={cpu} max={100} />
        </section>
        <section className="card meter">
          <span>RAM</span>
          <b>{usage ? `${ramPct.toFixed(0)}%` : "—"}</b>
          <small>{usage ? `${fmtBytes(usage.ram_used)} / ${fmtBytes(usage.ram_total)}` : ""}</small>
          <Sparkline values={ram} max={100} color="#4ea1ff" />
        </section>
      </div>

      <section className="card">
        <h3>{t("Placa de vídeo")}</h3>
        {gpus === null && <Loading text={t("Lendo a GPU…")} />}
        {gpuErr && <p className="muted">{t(gpuErr)}</p>}
        {g && (
          <>
            <p><b>{g.name}</b></p>
            <div className="monitor-grid">
              <div className="meter">
                <span>{t("Temperatura")}</span>
                <b className={tempClass(g.temp_c)}>{g.temp_c != null ? `${g.temp_c.toFixed(0)} °C` : "—"}</b>
                <Sparkline values={gTemp} max={100} color="#f5b301" />
              </div>
              <div className="meter">
                <span>{t("Uso da GPU")}</span>
                <b>{g.util_pct != null ? `${g.util_pct.toFixed(0)}%` : "—"}</b>
                <Sparkline values={gUtil} max={100} />
              </div>
            </div>
            <div className="stats">
              <div><span>{t("Memória de vídeo")}</span><b>{g.mem_used_mb != null && g.mem_total_mb != null ? `${g.mem_used_mb.toFixed(0)} / ${g.mem_total_mb.toFixed(0)} MB` : "—"}</b></div>
              <div><span>{t("Consumo")}</span><b>{g.power_w != null ? `${g.power_w.toFixed(1)} W` : "—"}</b></div>
              <div><span>{t("Clock")}</span><b>{g.clock_mhz != null ? `${g.clock_mhz.toFixed(0)} MHz` : "—"}</b></div>
              <div><span>{t("Ventoinha")}</span><b>{g.fan_pct != null ? `${g.fan_pct.toFixed(0)}%` : "—"}</b></div>
            </div>
          </>
        )}
        <p className="muted">{t("A temperatura da CPU não aparece porque o Windows não a expõe sem um driver de terceiros. Para a GPU usamos o nvidia-smi, que acompanha o driver da NVIDIA.")}</p>
      </section>
    </>
  );
}
