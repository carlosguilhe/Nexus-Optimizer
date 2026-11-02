import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search } from "lucide-react";
import { errText, fmtBytes } from "../types";
import { useT } from "../i18n";
import { useToast } from "../toast";
import { useConfirm } from "../confirm";
import { BtnLabel, Loading, PageHeader, ProgressBar } from "../components/ui";
import { tabIcon } from "../icons";
import { InstallPrograms } from "../components/Install";

interface App {
  package: string;
  label: string;
}

interface Program {
  id: string;
  name: string;
  version: string;
  publisher: string;
  size_kb: number;
  admin: boolean;
}

function Programs() {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const [list, setList] = useState<Program[] | null>(null);
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    invoke<Program[]>("programs").then(setList).catch(() => setList([]));
  }, []);

  async function uninstall(p: Program) {
    const ok = await confirm({
      title: t("Desinstalar {n}?", { n: p.name }),
      message: t("O desinstalador do próprio programa vai abrir e você conclui por lá. Isso não pode ser desfeito pelo Nexus."),
      confirmLabel: t("Abrir desinstalador"),
      danger: true,
    });
    if (!ok) return;
    setBusy(p.id);
    try {
      await invoke("uninstall_program", { id: p.id });
      toast(t("Desinstalador de {n} aberto.", { n: p.name }));
    } catch (e) {
      toast(t(errText(e)));
    }
    setBusy(null);
  }

  const shown = (list ?? []).filter((p) => `${p.name} ${p.publisher}`.toLowerCase().includes(q.trim().toLowerCase()));
  return (
    <section className="card">
      <h3>{t("Programas instalados")}</h3>
      <p className="muted">{t("Abre o desinstalador de cada programa. Instale e remova com cuidado: o Nexus só inicia o desinstalador oficial do programa.")}</p>
      <label className="search-wrap inline">
        <Search size={15} />
        <input className="search" value={q} onChange={(e) => setQ(e.currentTarget.value)} placeholder={t("Buscar programa…")} />
      </label>
      {list === null && <Loading text={t("Lendo programas instalados…")} />}
      {list && <small className="muted">{t("{n} programas", { n: shown.length })}</small>}
      <ul className="drivers">
        {shown.slice(0, 80).map((p) => (
          <li key={p.id}>
            <div>
              <b>{p.name}</b>
              <small>
                {p.publisher || "—"} {p.version && `· v${p.version}`} {p.size_kb > 0 && `· ${fmtBytes(p.size_kb * 1024)}`}
                {p.admin && <span className="chip">admin</span>}
              </small>
            </div>
            <button className="ghost small" disabled={busy !== null} onClick={() => uninstall(p)}>
              <BtnLabel busy={busy === p.id}>{t("Desinstalar")}</BtnLabel>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function AppsTab() {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const [apps, setApps] = useState<App[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState("");

  const load = useCallback(async () => {
    try {
      setApps(await invoke<App[]>("apps_list"));
    } catch (e) {
      setErr(t(errText(e)));
      setApps([]);
    }
  }, [t]);

  useEffect(() => {
    load();
  }, [load]);

  async function remove(a: App) {
    const ok = await confirm({
      title: t("Remover {n}?", { n: t(a.label) }),
      message: t("O aplicativo será removido deste usuário. Você pode reinstalá-lo depois pela Microsoft Store."),
      confirmLabel: t("Remover"),
      danger: true,
    });
    if (!ok) return;
    setBusy(a.package);
    setErr("");
    try {
      await invoke("apps_remove", { package: a.package });
      toast(t("{n} removido.", { n: t(a.label) }));
    } catch (e) {
      setErr(t(errText(e)));
    }
    await load();
    setBusy(null);
  }

  return (
    <>
      <PageHeader
        icon={tabIcon("Aplicativos")}
        title={t("Aplicativos")}
        desc={t("Instale programas populares, remova apps que o Windows e os fabricantes instalam e que quase ninguém usa, e desinstale o que não precisa mais.")}
      />
      <InstallPrograms />
      <h3 className="section-title">{t("Apps pré-instalados")}</h3>
      <p className="muted">{t("A lista é fechada: só aparecem aplicativos que são seguros remover. Qualquer um pode ser reinstalado pela Microsoft Store.")}</p>
      {err && <p className="error" onClick={() => setErr("")}>{err}</p>}
      {apps === null && <Loading text={t("Procurando aplicativos instalados…")} />}
      {busy && <ProgressBar label={t("Removendo aplicativo…")} />}
      {apps?.length === 0 && <p className="empty">{t("Nenhum aplicativo removível encontrado. Seu Windows já está limpo.")}</p>}
      <ul className="tweaks">
        {apps?.map((a) => (
          <li key={a.package}>
            <div className="info">
              <strong>{t(a.label)}</strong>
              <small className="path">{a.package}</small>
            </div>
            <button className="ghost" disabled={busy !== null} onClick={() => remove(a)}>
              <BtnLabel busy={busy === a.package}>{t("Remover")}</BtnLabel>
            </button>
          </li>
        ))}
      </ul>
      <Programs />
    </>
  );
}
