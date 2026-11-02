import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { UsageBars } from "../components/UsageBars";
import { MemoryTool } from "../components/Tools";
import { Loading, PageHeader } from "../components/ui";
import { useConfirm } from "../confirm";
import { useT } from "../i18n";
import { errText, fmtBytes } from "../types";
import { tabIcon } from "../icons";

interface Proc {
  name: string;
  count: number;
  mem_bytes: number;
  cpu_pct: number;
  protected: boolean;
}

export function ProcessesTab() {
  const t = useT();
  const confirm = useConfirm();
  const [procs, setProcs] = useState<Proc[] | null>(null);
  const [err, setErr] = useState("");
  const [msg, setMsg] = useState("");

  const load = useCallback(async () => {
    try {
      setProcs(await invoke<Proc[]>("processes"));
    } catch (e) {
      setErr(t(errText(e)));
      setProcs((p) => p ?? []);
    }
  }, [t]);

  useEffect(() => {
    load();
    const id = setInterval(load, 3000);
    return () => clearInterval(id);
  }, [load]);

  async function kill(p: Proc) {
    const ok = await confirm({
      title: t("Encerrar {n}?", { n: p.name }),
      message: p.count > 1 ? t("{n} processos serão encerrados. Dados não salvos nesse programa serão perdidos.", { n: p.count }) : t("1 processo será encerrado. Dados não salvos nesse programa serão perdidos."),
      confirmLabel: t("Encerrar"),
      danger: true,
    });
    if (!ok) return;
    setErr("");
    setMsg("");
    try {
      const n = await invoke<number>("kill_process", { name: p.name });
      setMsg(t("{name}: {n} processo(s) encerrado(s).", { name: p.name, n }));
    } catch (e) {
      setErr(t(errText(e)));
    }
    load();
  }

  return (
    <>
      <PageHeader
        icon={tabIcon("Processos")}
        title={t("Processos")}
        desc={t("Os programas que mais usam memória agora. Antes de jogar, feche o que você não precisa. Processos do Windows e do próprio Nexus são protegidos e não podem ser encerrados aqui.")}
      />
      <section className="card">
        <UsageBars />
      </section>
      {err && <p className="error" onClick={() => setErr("")}>{err}</p>}
      {msg && <p className="ok">{msg}</p>}
      <section className="card">
        {procs === null && <Loading text={t("Lendo processos…")} />}
        {procs && (
          <table>
            <thead>
              <tr><th>{t("Programa")}</th><th>{t("Inst.")}</th><th>{t("Memória")}</th><th>CPU</th><th></th></tr>
            </thead>
            <tbody>
              {procs.map((p) => (
                <tr key={p.name}>
                  <td>{p.name}</td>
                  <td>{p.count}</td>
                  <td>{fmtBytes(p.mem_bytes)}</td>
                  <td>{p.cpu_pct.toFixed(1)}%</td>
                  <td className="right">
                    {p.protected ? <span className="chip">{t("sistema")}</span> : <button className="ghost small" onClick={() => kill(p)}>{t("Encerrar")}</button>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
      <MemoryTool />
    </>
  );
}
