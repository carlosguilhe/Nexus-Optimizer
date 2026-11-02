import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Download } from "lucide-react";
import { errText, fmtBytes } from "../types";
import { useT } from "../i18n";
import { useConfirm } from "../confirm";
import { BtnLabel, ProgressBar } from "./ui";

interface Progress {
  stage: "download" | "verify" | "install";
  done: number;
  total: number;
}

/** Baixa a versão nova do GitHub, confere o hash e instala. O app fecha e reabre sozinho. */
export function UpdateNow({ latest, onError }: { latest: string; onError?: (msg: string) => void }) {
  const t = useT();
  const confirm = useConfirm();
  const [busy, setBusy] = useState(false);
  const [p, setP] = useState<Progress | null>(null);

  useEffect(() => {
    const un = listen<Progress>("update_progress", (e) => setP(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function run() {
    const ok = await confirm({
      title: t("Atualizar para {v}?", { v: latest }),
      message: t("O Nexus baixa a nova versão, confere a integridade do arquivo, fecha, instala e abre de novo sozinho. Seus ajustes e configurações são mantidos."),
      confirmLabel: t("Atualizar agora"),
    });
    if (!ok) return;
    setBusy(true);
    setP(null);
    try {
      await invoke("update_install");
    } catch (e) {
      onError?.(t(errText(e)));
      setBusy(false);
    }
  }

  const label =
    p?.stage === "verify"
      ? t("Conferindo a integridade do arquivo…")
      : p?.stage === "install"
        ? t("Instalando… o Nexus vai reabrir sozinho.")
        : p && p.total > 0
          ? `${t("Baixando atualização…")} ${fmtBytes(p.done)} / ${fmtBytes(p.total)}`
          : t("Baixando atualização…");

  return (
    <>
      <button disabled={busy} onClick={run}>
        <BtnLabel busy={busy}>
          <Download size={15} /> {t("Atualizar agora")}
        </BtnLabel>
      </button>
      {busy && (
        <div className="update-progress">
          <ProgressBar label={label} value={p?.stage === "download" && p.total > 0 ? p.done / p.total : p ? 1 : undefined} />
        </div>
      )}
    </>
  );
}
