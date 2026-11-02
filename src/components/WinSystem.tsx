import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-react";
import { errText, Tweak } from "../types";
import { useT } from "../i18n";
import { useToast } from "../toast";
import { useConfirm } from "../confirm";
import { BtnLabel, Loading, ProgressBar, Switch } from "./ui";

// ------------------------------------------------------------ Windows Update

type Mode = "padrao" | "seguranca" | "pausado";

const MODES: { id: Mode; title: string; desc: string; on: string[]; off: string[] }[] = [
  {
    id: "padrao",
    title: "Padrão",
    desc: "O Windows decide: baixa e instala tudo sozinho.",
    on: [],
    off: ["wu-security-only", "wu-pause"],
  },
  {
    id: "seguranca",
    title: "Só segurança",
    desc: "Correções de segurança normais; adia por 1 ano as atualizações grandes de versão.",
    on: ["wu-security-only"],
    off: ["wu-pause"],
  },
  {
    id: "pausado",
    title: "Manual",
    desc: "Não instala nada sozinho. Você atualiza quando quiser, pelo Windows Update.",
    on: ["wu-pause"],
    off: ["wu-security-only"],
  },
];

const EXTRAS = ["wu-no-auto-reboot", "wu-no-drivers"];

export function WindowsUpdateCard({ tweaks, admin, refresh }: { tweaks: Tweak[]; admin: boolean; refresh: () => Promise<void> }) {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState("");

  const on = (id: string) => tweaks.find((x) => x.id === id)?.applied ?? false;
  const mode: Mode = on("wu-pause") ? "pausado" : on("wu-security-only") ? "seguranca" : "padrao";

  async function setTweak(id: string, apply: boolean) {
    const now = on(id);
    if (now === apply) return;
    await invoke(apply ? "apply_tweak" : "revert_tweak", { id });
  }

  async function choose(m: (typeof MODES)[number]) {
    if (m.id === mode) return;
    if (m.id === "pausado") {
      const ok = await confirm({
        title: t("Parar as atualizações automáticas?"),
        message: t("Com o modo Manual o PC fica sem correções de segurança até você atualizar à mão. Lembre-se de verificar o Windows Update de vez em quando."),
        confirmLabel: t("Usar modo Manual"),
        danger: true,
      });
      if (!ok) return;
    }
    setBusy(m.id);
    setErr("");
    try {
      for (const id of m.off) await setTweak(id, false);
      for (const id of m.on) await setTweak(id, true);
      toast(t("Windows Update: modo {n}.", { n: t(m.title) }));
    } catch (e) {
      setErr(t(errText(e)));
    }
    await refresh();
    setBusy(null);
  }

  async function toggleExtra(id: string, next: boolean) {
    setBusy(id);
    setErr("");
    try {
      await setTweak(id, next);
    } catch (e) {
      setErr(t(errText(e)));
    }
    await refresh();
    setBusy(null);
  }

  return (
    <section className="card">
      <h3>{t("Controle do Windows Update")}</h3>
      <p className="muted">
        {t("Escolha como o Windows se atualiza. Tudo é feito por políticas oficiais do Windows e pode ser desfeito voltando ao modo Padrão.")}
      </p>
      {!admin && <p className="warn">{t("Exige administrador.")}</p>}
      {err && <p className="error" onClick={() => setErr("")}>{err}</p>}
      <div className="mode-grid">
        {MODES.map((m) => (
          <button
            key={m.id}
            className={`mode${mode === m.id ? " selected" : ""}`}
            disabled={!admin || busy !== null}
            onClick={() => choose(m)}
            aria-pressed={mode === m.id}
          >
            <b>
              <BtnLabel busy={busy === m.id}>{t(m.title)}</BtnLabel>
            </b>
            <small>{t(m.desc)}</small>
          </button>
        ))}
      </div>
      <ul className="tweaks">
        {EXTRAS.map((id) => {
          const tw = tweaks.find((x) => x.id === id);
          if (!tw) return null;
          return (
            <li key={id}>
              <div className="info">
                <strong>{t(tw.name)}</strong>
                <small>{t(tw.description)}</small>
              </div>
              <Switch checked={tw.applied} disabled={!admin || busy !== null} onChange={(v) => toggleExtra(id, v)} />
            </li>
          );
        })}
      </ul>
      <div className="row">
        <button className="ghost" onClick={() => invoke("open_target", { key: "winupdate" })}>
          <RefreshCw size={14} /> {t("Abrir o Windows Update")}
        </button>
      </div>
    </section>
  );
}

// ----------------------------------------------------- Recursos do Windows

interface Feature {
  id: string;
  label: string;
  description: string;
  warning: string;
  state: string;
}

export function FeaturesCard({ admin }: { admin: boolean }) {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const [list, setList] = useState<Feature[] | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [restart, setRestart] = useState(false);

  async function load() {
    if (!admin) {
      setList([]);
      return;
    }
    try {
      setList(await invoke<Feature[]>("features_list"));
      setErr("");
    } catch (e) {
      setErr(t(errText(e)));
      setList([]);
    }
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [admin]);

  async function toggle(f: Feature) {
    const enabled = f.state === "Enabled" || f.state === "EnablePending";
    if (!enabled && f.warning) {
      const ok = await confirm({
        title: t("Ativar {n}?", { n: t(f.label) }),
        message: t(f.warning),
        confirmLabel: t("Ativar"),
        danger: true,
      });
      if (!ok) return;
    }
    setBusy(f.id);
    setErr("");
    try {
      const needsRestart = await invoke<boolean>("feature_set", { id: f.id, enable: !enabled });
      if (needsRestart) setRestart(true);
      toast(`${enabled ? t("Desativado") : t("Ativado")}: ${t(f.label)}`);
    } catch (e) {
      setErr(t(errText(e)));
    }
    await load();
    setBusy(null);
  }

  return (
    <section className="card">
      <h3>{t("Recursos do Windows")}</h3>
      <p className="muted">
        {t("Liga e desliga componentes opcionais do Windows, como o .NET 3.5 que jogos antigos pedem. Ativar um recurso pode baixar arquivos e levar alguns minutos.")}
      </p>
      {!admin && <p className="warn">{t("Exige administrador.")}</p>}
      {err && <p className="error" onClick={() => setErr("")}>{err}</p>}
      {restart && <p className="warn">{t("O Windows pede reiniciar o PC para concluir a mudança.")}</p>}
      {admin && list === null && <Loading text={t("Lendo recursos do Windows…")} />}
      {busy && <ProgressBar label={t("Alterando o recurso… isso pode demorar")} />}
      <ul className="tweaks">
        {list?.map((f) => {
          const unavailable = f.state === "Unavailable";
          return (
            <li key={f.id}>
              <div className="info">
                <strong>{t(f.label)}</strong>
                <small>{unavailable ? t("Não disponível nesta edição do Windows.") : t(f.description)}</small>
              </div>
              <Switch
                checked={f.state === "Enabled" || f.state === "EnablePending"}
                disabled={!admin || busy !== null || unavailable}
                onChange={() => toggle(f)}
              />
            </li>
          );
        })}
      </ul>
    </section>
  );
}
