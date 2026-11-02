import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, Sparkles } from "lucide-react";
import { Tweak } from "../types";
import { useT } from "../i18n";
import { PRESETS, suggestPreset } from "../presets";
import { BtnLabel, Loading, ProgressBar } from "./ui";

interface Props {
  tweaks: Tweak[];
  admin: boolean;
  onApply: (ids: string[]) => Promise<void>;
  onFinish: () => void;
}

/** Assistente de primeiro uso: lê o PC, sugere um perfil e aplica só o que você aceitar. */
export function Wizard({ tweaks, admin, onApply, onFinish }: Props) {
  const t = useT();
  const [step, setStep] = useState(0);
  const [info, setInfo] = useState<unknown>(null);
  const [choice, setChoice] = useState("equilibrado");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke("system_info")
      .then((i) => {
        setInfo(i);
        setChoice(suggestPreset(i));
      })
      .catch(() => setInfo({}));
  }, []);

  const preset = PRESETS.find((p) => p.id === choice) ?? PRESETS[0];
  const usable = preset.tweaks.filter((id) => tweaks.some((x) => x.id === id && !x.applied && (!x.admin || admin)));

  async function apply() {
    setBusy(true);
    await onApply(usable);
    setBusy(false);
    onFinish();
  }

  return (
    <div className="modal-backdrop wizard">
      <div className="modal wide" role="dialog" aria-modal="true">
        <span className="modal-icon">
          <Sparkles size={24} />
        </span>
        {step === 0 && (
          <>
            <h3>{t("Bem-vindo ao Nexus Optimizer")}</h3>
            <div className="modal-msg">
              <p>{t("Vamos configurar o seu PC em um minuto. O Nexus só aplica ajustes que você confirmar, e todos podem ser desfeitos a qualquer momento.")}</p>
              {info === null ? <Loading text={t("Lendo o seu hardware…")} /> : <p className="ok">{t("Hardware lido. Já temos uma sugestão para você.")}</p>}
            </div>
            <div className="modal-actions">
              <button className="ghost" onClick={onFinish}>{t("Pular")}</button>
              <button disabled={info === null} onClick={() => setStep(1)}>
                {t("Continuar")} <ArrowRight size={15} />
              </button>
            </div>
          </>
        )}
        {step === 1 && (
          <>
            <h3>{t("Escolha um perfil")}</h3>
            <div className="presets">
              {PRESETS.map((p) => (
                <button key={p.id} className={`preset ${choice === p.id ? "on" : ""}`} onClick={() => setChoice(p.id)}>
                  <b>
                    {t(p.name)} {p.id === suggestPreset(info) && <span className="chip cat">{t("sugerido")}</span>}
                  </b>
                  <small>{t(p.desc)}</small>
                </button>
              ))}
            </div>
            {busy && <ProgressBar label={t("Aplicando o perfil…")} />}
            <div className="modal-actions">
              <button className="ghost" disabled={busy} onClick={() => setStep(0)}>{t("Voltar")}</button>
              <button disabled={busy || usable.length === 0} onClick={apply}>
                <BtnLabel busy={busy}>{t("Aplicar {n} ajuste(s)", { n: usable.length })}</BtnLabel>
              </button>
              <button className="ghost" disabled={busy} onClick={onFinish}>{t("Agora não")}</button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
