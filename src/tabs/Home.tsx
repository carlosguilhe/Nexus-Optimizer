import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Download,
  Gamepad2,
  Gauge,
  Globe,
  History,
  Info,
  LucideIcon,
  MemoryStick,
  Package,
  Rocket,
  Sparkles,
  Timer,
  Undo2,
  Wand2,
  Wrench,
} from "lucide-react";
import { errText, fmtBytes, Tweak } from "../types";
import { UsageBars } from "../components/UsageBars";
import { BtnLabel, Loading, PageHeader, Spinner } from "../components/ui";
import { useToast } from "../toast";
import { useT, T } from "../i18n";
import { tabIcon } from "../icons";
import { PRESETS, Preset } from "../presets";

/* eslint-disable @typescript-eslint/no-explicit-any */
type SysInfo = Record<string, any>;

const arr = (v: any): any[] => (v == null ? [] : Array.isArray(v) ? v : [v]);

interface Finding {
  level: "warn" | "info" | "ok";
  title: string;
  text: string;
}

function analyze(info: SysInfo, t: T): Finding[] {
  const f: Finding[] = [];
  const mem = arr(info.mem);
  const totalGb = mem.reduce((s, m) => s + (m.Capacity ?? 0), 0) / 1024 ** 3;

  if (mem.length === 1) {
    f.push({
      level: "warn",
      title: t("Memória RAM em um único pente"),
      text: t("Com um só pente, a RAM roda em canal único. Dois pentes iguais (dual channel) costumam dar ganho grande de FPS, principalmente em CPUs/vídeo integrado."),
    });
  } else if (mem.length >= 2) {
    f.push({ level: "ok", title: t("{n} pentes de RAM", { n: mem.length }), text: t("Provável dual channel.") });
  }
  if (mem.some((m) => m.ConfiguredClockSpeed && m.Speed && m.ConfiguredClockSpeed < m.Speed)) {
    f.push({
      level: "warn",
      title: t("RAM abaixo da velocidade nominal"),
      text: t("A memória roda mais devagar do que o pente suporta. Ative o perfil XMP/EXPO na BIOS."),
    });
  }
  if (totalGb > 0 && totalGb < 16) {
    f.push({
      level: "info",
      title: t("{n} GB de RAM", { n: totalGb.toFixed(0) }),
      text: t("Jogos modernos pedem 16 GB. Com menos, feche navegador e apps em segundo plano ao jogar."),
    });
  }
  if (/equilibr|balanced|economia|saver/i.test(info.plan ?? "")) {
    f.push({
      level: "warn",
      title: t("Plano de energia econômico"),
      text: t("O Windows limita o desempenho da CPU. Aplique 'Plano de energia Desempenho Máximo' na aba Energia."),
    });
  }
  if (arr(info.disks).some((d) => d.MediaType === "HDD")) {
    f.push({
      level: "info",
      title: t("HD mecânico detectado"),
      text: t("Instale seus jogos em SSD e não desative o SysMain se o Windows estiver em HD."),
    });
  }
  for (const g of arr(info.gpus)) {
    if (g.CurrentRefreshRate) {
      f.push({
        level: "info",
        title: `${g.Name}: ${g.CurrentRefreshRate} Hz`,
        text: t("Confirme em Configurações > Tela > Taxa de atualização que está no máximo do seu monitor."),
      });
    }
  }
  if (info.battery) {
    f.push({
      level: "info",
      title: t("Notebook"),
      text: t("Jogue na tomada e com plano de energia de alto desempenho; na bateria o desempenho cai muito."),
    });
  }
  return f;
}

const LEVEL_ICON: Record<Finding["level"], LucideIcon> = { warn: AlertTriangle, info: Info, ok: CheckCircle2 };

interface Tile {
  key: string;
  icon: LucideIcon;
  title: string;
  sub: string;
  to?: string;
  action?: () => Promise<void>;
}

interface Props {
  tweaks: Tweak[];
  admin: boolean;
  busy: string | null;
  update: { latest: string } | null;
  onApplySafe: () => void;
  onRevertAll: () => void;
  onApplyPreset: (p: Preset) => void;
  goTo: (tab: string) => void;
}

export function Home({ tweaks, admin, busy, update, onApplySafe, onRevertAll, onApplyPreset, goTo }: Props) {
  const t = useT();
  const toast = useToast();
  const [info, setInfo] = useState<SysInfo | null>(null);
  const [err, setErr] = useState("");
  const [running, setRunning] = useState<string | null>(null);

  useEffect(() => {
    invoke<SysInfo>("system_info").then(setInfo).catch((e) => setErr(t(errText(e))));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const active = tweaks.filter((x) => x.applied).length;
  const safePending = tweaks.filter((x) => x.risk === "seguro" && !x.applied && (!x.admin || admin)).length;
  const cpu = arr(info?.cpu)[0];
  const os = arr(info?.os)[0];
  const mem = arr(info?.mem);
  const totalGb = mem.reduce((s, m) => s + (m.Capacity ?? 0), 0) / 1024 ** 3;

  const tiles: Tile[] = [
    { key: "speed", icon: Gauge, title: t("Teste de velocidade"), sub: t("Ping, download e upload"), to: "Velocidade" },
    { key: "bench", icon: Timer, title: t("Benchmark de FPS"), sub: t("Meça antes e depois"), to: "Benchmark" },
    { key: "gm", icon: Gamepad2, title: t("Modo Jogo"), sub: t("Perfis por jogo, automáticos"), to: "Jogos" },
    { key: "clean", icon: Sparkles, title: t("Limpar o PC"), sub: t("Temporários, caches e Lixeira"), to: "Sistema" },
    {
      key: "ram",
      icon: MemoryStick,
      title: t("Liberar RAM"),
      sub: t("Ação rápida, efeito temporário"),
      action: async () => {
        const [before, after] = await invoke<[number, number]>("free_memory");
        toast(t("Memória: {size} liberados agora.", { size: fmtBytes(Math.max(0, before - after)) }));
      },
    },
    { key: "proc", icon: Activity, title: t("Processos"), sub: t("Feche o que pesa"), to: "Processos" },
    { key: "boot", icon: Rocket, title: t("Inicialização"), sub: t("Programas que abrem com o Windows"), to: "Inicialização" },
    { key: "net", icon: Globe, title: t("Rede e DNS"), sub: t("Latência, rota e servidor DNS"), to: "Rede" },
    { key: "apps", icon: Package, title: t("Aplicativos"), sub: t("Remover apps inúteis"), to: "Aplicativos" },
    { key: "maint", icon: Wrench, title: t("Manutenção"), sub: t("Discos, SFC e drivers"), to: "Manutenção" },
    {
      key: "restore",
      icon: History,
      title: t("Ponto de restauração"),
      sub: admin ? t("Criar agora") : t("Exige administrador"),
      action: async () => {
        toast(t(await invoke<string>("restore_point")));
      },
    },
  ];

  async function runTile(tile: Tile) {
    if (tile.to) return goTo(tile.to);
    if (!tile.action) return;
    setRunning(tile.key);
    try {
      await tile.action();
    } catch (e) {
      toast(t(errText(e)));
    }
    setRunning(null);
  }

  return (
    <>
      <PageHeader
        icon={tabIcon("Início")}
        title={t("Painel")}
        desc={t("{a} de {b} ajustes ativos · {c} ajustes seguros pendentes", { a: active, b: tweaks.length, c: safePending })}
      />

      {update && (
        <p className="banner">
          <Download size={16} /> {t("Nova versão disponível: {v}", { v: update.latest })}
          <button className="ghost small" onClick={() => invoke("open_target", { key: "releases" })}>
            {t("Baixar")}
          </button>
        </p>
      )}

      <div className="hero">
        <div>
          <b>{t("Comece pelo recomendado")}</b>
          <p className="muted">{t("Aplica só os ajustes de risco baixo. Cada um pode ser desfeito depois.")}</p>
        </div>
        <div className="row">
          <button className="ghost" disabled={busy !== null || active === 0} onClick={onRevertAll}>
            <Undo2 size={15} /> {t("Reverter tudo")}
          </button>
          <button disabled={busy !== null || safePending === 0} onClick={onApplySafe}>
            <BtnLabel busy={busy === "all"}>
              {busy === "all" ? t("Aplicando…") : <><Wand2 size={15} /> {t("Aplicar todos os seguros")}</>}
            </BtnLabel>
          </button>
        </div>
      </div>

      <h3 className="section-title">{t("Perfis prontos")}</h3>
      <div className="presets">
        {PRESETS.map((p) => (
          <button key={p.id} className="preset" disabled={busy !== null} onClick={() => onApplyPreset(p)}>
            <b>{t(p.name)}</b>
            <small>{t(p.desc)}</small>
          </button>
        ))}
      </div>

      <h3 className="section-title">{t("Atalhos")}</h3>
      <div className="tiles">
        {tiles.map((tile) => {
          const Icon = tile.icon;
          const disabled = running !== null || (tile.key === "restore" && !admin);
          return (
            <button key={tile.key} className="tile" disabled={disabled} onClick={() => runTile(tile)}>
              <span className="tile-icon">{running === tile.key ? <Spinner size={20} /> : <Icon size={22} />}</span>
              <span className="tile-text">
                <b>{tile.title}</b>
                <small>{tile.sub}</small>
              </span>
            </button>
          );
        })}
      </div>

      <section className="card">
        <h3>{t("Uso agora")}</h3>
        <UsageBars />
      </section>

      <section className="card">
        <h3>{t("Seu PC")}</h3>
        {err && <p className="error">{err}</p>}
        {!info && !err && <Loading text={t("Lendo hardware…")} />}
        {info && (
          <div className="specs">
            <div><span>{t("Sistema")}</span><b>{os?.Caption} ({os?.BuildNumber})</b></div>
            <div><span>{t("Processador")}</span><b>{cpu?.Name?.trim()} · {cpu?.NumberOfCores}N/{cpu?.NumberOfLogicalProcessors}T</b></div>
            {arr(info.gpus).map((g, i) => (
              <div key={i}><span>{t("Vídeo")}</span><b>{g.Name} · driver {g.DriverVersion}</b></div>
            ))}
            <div>
              <span>{t("Memória")}</span>
              <b>{totalGb.toFixed(0)} GB · {t("{n} pente(s)", { n: mem.length })} · {mem[0]?.ConfiguredClockSpeed ?? mem[0]?.Speed} MHz</b>
            </div>
            {arr(info.disks)
              .filter((d) => d.Size > 32 * 1024 ** 3)
              .map((d, i) => (
                <div key={i}><span>{t("Disco")}</span><b>{d.FriendlyName} · {d.MediaType} · {fmtBytes(d.Size)}</b></div>
              ))}
            <div><span>{t("Energia")}</span><b>{String(info.plan ?? "").replace(/^.*?:\s*/, "")}</b></div>
          </div>
        )}
      </section>

      {info && (
        <section className="card">
          <h3>{t("Diagnóstico")}</h3>
          <ul className="findings">
            {analyze(info, t).map((f, i) => {
              const Icon = LEVEL_ICON[f.level];
              return (
                <li key={i} className={f.level}>
                  <Icon size={18} />
                  <div>
                    <b>{f.title}</b>
                    <p>{f.text}</p>
                  </div>
                </li>
              );
            })}
          </ul>
          <p className="muted">{t("Estes alertas podem ter mais efeito que qualquer ajuste de registro. Hardware e BIOS vêm primeiro.")}</p>
        </section>
      )}
    </>
  );
}
