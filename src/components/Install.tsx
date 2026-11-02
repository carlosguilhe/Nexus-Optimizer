import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowUpCircle, CheckCircle2, Download } from "lucide-react";
import { errText } from "../types";
import { useT } from "../i18n";
import { useToast } from "../toast";
import { useConfirm } from "../confirm";
import { BtnLabel, Loading, ProgressBar } from "./ui";
import { GameIcon } from "./GameIcon";

interface Pkg {
  id: string;
  name: string;
  category: string;
  installed: boolean;
}

interface WingetEvent {
  id: string;
  line: string;
  progress: number | null;
}

/** Instala e atualiza programas pelo winget, a partir de uma lista fechada. */
export function InstallPrograms() {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const [list, setList] = useState<Pkg[] | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [live, setLive] = useState<WingetEvent | null>(null);

  const load = useCallback(async () => {
    try {
      setList(await invoke<Pkg[]>("winget_list"));
      setErr("");
    } catch (e) {
      setErr(t(errText(e)));
      setList([]);
    }
  }, [t]);

  useEffect(() => {
    load();
    const un = listen<WingetEvent>("winget", (e) => setLive(e.payload));
    return () => {
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function run(command: "winget_install" | "winget_upgrade", p: Pkg) {
    setBusy(p.id);
    setLive(null);
    try {
      await invoke(command, { id: p.id });
      toast(command === "winget_install" ? t("{n} instalado.", { n: p.name }) : t("{n} atualizado.", { n: p.name }));
    } catch (e) {
      setErr(`${p.name}: ${t(errText(e))}`);
    }
    setBusy(null);
    setLive(null);
    await load();
  }

  async function upgradeAll() {
    const ok = await confirm({
      title: t("Atualizar todos os programas?"),
      message: t("O winget vai atualizar tudo o que tiver versão nova, inclusive programas que não estão nesta lista. Pode levar vários minutos."),
      confirmLabel: t("Atualizar tudo"),
    });
    if (!ok) return;
    setBusy("*");
    setLive(null);
    try {
      await invoke("winget_upgrade_all");
      toast(t("Programas atualizados."));
    } catch (e) {
      setErr(t(errText(e)));
    }
    setBusy(null);
    setLive(null);
    await load();
  }

  const categories = [...new Set((list ?? []).map((p) => p.category))];
  const working = busy !== null;
  const current = live && (busy === live.id || busy === "*") ? live : null;

  return (
    <section className="card">
      <h3>{t("Instalar programas")}</h3>
      <p className="muted">
        {t("Instala e atualiza programas populares pelo winget, o gerenciador oficial da Microsoft. Só aparecem programas de uma lista fechada, baixados das fontes do próprio winget.")}
      </p>
      {err && <p className="error" onClick={() => setErr("")}>{err}</p>}
      {list === null && <Loading text={t("Consultando o winget…")} />}
      {working && (
        <div className="install-progress">
          <ProgressBar
            label={
              current?.line
                ? current.line
                : busy === "*"
                  ? t("Atualizando programas…")
                  : t("Instalando…")
            }
            value={current?.progress != null ? current.progress / 100 : undefined}
          />
        </div>
      )}
      {list && list.length > 0 && (
        <div className="row">
          <button className="ghost" disabled={working} onClick={upgradeAll}>
            <BtnLabel busy={busy === "*"}>
              <ArrowUpCircle size={15} /> {t("Atualizar todos os programas")}
            </BtnLabel>
          </button>
        </div>
      )}
      {categories.map((cat) => (
        <div key={cat}>
          <h4 className="install-cat">{t(cat)}</h4>
          <ul className="drivers">
            {list!
              .filter((p) => p.category === cat)
              .map((p) => (
                <li key={p.id}>
                  <div className="pkg-row">
                    <GameIcon url={`/pkg-icons/${p.id.replace(/[^A-Za-z0-9.-]/g, "_")}.png`} name={p.name} size={40} className="pkg-icon" />
                    <div>
                      <b>{p.name}</b>
                      <small>
                        {p.id}
                        {p.installed && (
                          <span className="chip risk-seguro">
                            <CheckCircle2 size={11} /> {t("instalado")}
                          </span>
                        )}
                      </small>
                    </div>
                  </div>
                  {p.installed ? (
                    <button className="ghost small" disabled={working} onClick={() => run("winget_upgrade", p)}>
                      <BtnLabel busy={busy === p.id}>{t("Atualizar")}</BtnLabel>
                    </button>
                  ) : (
                    <button className="small" disabled={working} onClick={() => run("winget_install", p)}>
                      <BtnLabel busy={busy === p.id}>
                        <Download size={13} /> {t("Instalar")}
                      </BtnLabel>
                    </button>
                  )}
                </li>
              ))}
          </ul>
        </div>
      ))}
    </section>
  );
}
