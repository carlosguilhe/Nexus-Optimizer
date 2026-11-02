import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errText } from "../types";
import { useT } from "../i18n";
import { BtnLabel, Loading, PageHeader } from "../components/ui";
import { tabIcon } from "../icons";

interface Item {
  loc: number;
  name: string;
  command: string;
  scope: string;
  enabled: boolean;
  admin: boolean;
}

export function StartupTab({ admin }: { admin: boolean }) {
  const t = useT();
  const [items, setItems] = useState<Item[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState("");

  const load = useCallback(async () => {
    try {
      setItems(await invoke<Item[]>("startup_list"));
    } catch (e) {
      setErr(t(errText(e)));
      setItems((i) => i ?? []);
    }
  }, [t]);

  useEffect(() => {
    load();
  }, [load]);

  async function toggle(it: Item) {
    setBusy(`${it.loc}:${it.name}`);
    setErr("");
    try {
      await invoke("startup_set", { loc: it.loc, name: it.name, enabled: !it.enabled });
    } catch (e) {
      setErr(t(errText(e)));
    }
    await load();
    setBusy(null);
  }

  const active = items?.filter((i) => i.enabled).length ?? 0;

  return (
    <>
      <PageHeader
        icon={tabIcon("Inicialização")}
        title={t("Inicialização")}
        desc={t("Programas que abrem junto com o Windows. Desativar acelera o boot e libera memória. Nada é apagado: o item só é marcado como desativado, igual ao Gerenciador de Tarefas, e você pode reativar quando quiser.")}
      />
      {items === null ? <Loading text={t("Lendo programas de inicialização…")} /> : <p className="muted">{t("{a} de {b} ativos", { a: active, b: items.length })}</p>}
      {err && <p className="error" onClick={() => setErr("")}>{err}</p>}
      {!admin && <p className="warn">{t("Itens do \"Sistema\" exigem administrador para serem alterados.")}</p>}
      <ul className="tweaks">
        {items?.map((it) => {
          const key = `${it.loc}:${it.name}`;
          return (
            <li key={key} className={it.enabled ? "on" : ""}>
              <div className="info">
                <div className="title">
                  <strong>{it.name}</strong>
                  <span className="chip">{t(it.scope)}</span>
                  <span className="chip">{it.enabled ? t("ativo") : t("desativado")}</span>
                </div>
                <small className="path">{it.command}</small>
              </div>
              <button className={it.enabled ? "" : "ghost"} disabled={busy !== null || (it.admin && !admin)} onClick={() => toggle(it)}>
                <BtnLabel busy={busy === key}>{it.enabled ? t("Desativar") : t("Ativar")}</BtnLabel>
              </button>
            </li>
          );
        })}
        {items?.length === 0 && <p className="empty">{t("Nenhum programa de inicialização encontrado.")}</p>}
      </ul>
    </>
  );
}
