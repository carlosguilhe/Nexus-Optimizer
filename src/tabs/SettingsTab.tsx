import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Download, ExternalLink, FileUp, Trash2 } from "lucide-react";
import { errText } from "../types";
import { LANGUAGES, useT } from "../i18n";
import { useSettings } from "../settings";
import { THEMES } from "../theme";
import { ThemeThumb } from "../components/ThemeThumb";
import { BtnLabel, Loading, PageHeader, Switch } from "../components/ui";
import { useToast } from "../toast";
import { useConfirm } from "../confirm";
import { tabIcon } from "../icons";


interface Entry {
  ts: number;
  kind: string;
  text: string;
}

interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
}

function Row({ title, desc, children }: { title: string; desc?: string; children: React.ReactNode }) {
  return (
    <div className="setting-row">
      <div>
        <b>{title}</b>
        {desc && <small>{desc}</small>}
      </div>
      {children}
    </div>
  );
}

export function SettingsTab() {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const { settings, update } = useSettings();
  const [version, setVersion] = useState("");
  const [upd, setUpd] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [updErr, setUpdErr] = useState("");
  const [log, setLog] = useState<Entry[] | null>(null);
  const [exported, setExported] = useState("");
  const [importText, setImportText] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const loadLog = useCallback(() => invoke<Entry[]>("activity_list", { limit: 100 }).then(setLog).catch(() => setLog([])), []);

  useEffect(() => {
    invoke<string>("app_version").then(setVersion);
    loadLog();
  }, [loadLog]);

  async function checkNow() {
    setChecking(true);
    setUpdErr("");
    try {
      setUpd(await invoke<UpdateInfo>("check_update"));
    } catch (e) {
      setUpdErr(t(errText(e)));
    }
    setChecking(false);
  }

  async function doExport() {
    setBusy("export");
    try {
      const text = await invoke<string>("config_export");
      setExported(text);
      try {
        await navigator.clipboard.writeText(text);
      } catch {
        /* sem permissão de área de transferência: o texto continua na caixa */
      }
      toast(t("Configuração exportada para Documentos\\Nexus Backups e copiada."));
      loadLog();
    } catch (e) {
      toast(t(errText(e)));
    }
    setBusy(null);
  }

  async function doImport() {
    const ok = await confirm({
      title: t("Importar configuração?"),
      message: t("As configurações do app serão substituídas e os ajustes do arquivo serão aplicados."),
      confirmLabel: t("Importar"),
    });
    if (!ok) return;
    setBusy("import");
    try {
      const r = await invoke<{ applied: number; failed: string[] }>("config_import", { text: importText });
      toast(t("Importado: {n} ajuste(s) aplicados.", { n: r.applied }) + (r.failed.length ? ` ${r.failed.length} ✗` : ""));
      setImportText("");
      loadLog();
    } catch (e) {
      toast(t(errText(e)));
    }
    setBusy(null);
  }

  async function clearLog() {
    const ok = await confirm({
      title: t("Limpar o histórico?"),
      message: t("O registro de atividades será apagado. Os ajustes aplicados continuam valendo."),
      confirmLabel: t("Limpar"),
      danger: true,
    });
    if (!ok) return;
    await invoke("activity_clear");
    loadLog();
  }

  const CLEAN_OPTIONS = [0, 3, 7, 14, 30];

  return (
    <>
      <PageHeader icon={tabIcon("Configurações")} title={t("Configurações")} desc={t("Aparência, idioma, comportamento do app, backup e histórico.")} />

      <section className="card">
        <h3>{t("Aparência")}</h3>
        <p className="muted">{t("Escolha o tema. A mudança é imediata e fica salva.")}</p>
        <div className="theme-grid">
          {THEMES.map((th) => {
            const active = settings.theme === th.id;
            return (
              <div key={th.id} className={`theme-card ${active ? "active" : ""}`}>
                <ThemeThumb theme={th} />
                <div className="theme-foot">
                  <b>{t(th.name)}</b>
                  <button className={active ? "ghost" : ""} disabled={active} onClick={() => update({ theme: th.id })}>
                    {active ? (
                      <>
                        <Check size={14} /> {t("Em uso")}
                      </>
                    ) : (
                      t("Selecionar")
                    )}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section className="card">
        <h3>{t("Idioma")}</h3>
        <div className="row">
          {LANGUAGES.map((l) => (
            <button key={l.id} className={settings.lang === l.id ? "" : "ghost"} onClick={() => update({ lang: l.id })}>
              {settings.lang === l.id && <Check size={14} />} {l.label}
            </button>
          ))}
        </div>
        <p className="muted">{t("Algumas mensagens técnicas do Windows continuam no idioma original.")}</p>
      </section>

      <section className="card">
        <h3>{t("Comportamento")}</h3>
        <Row title={t("Minimizar para a bandeja")} desc={t("Fechar a janela mantém o Nexus rodando em segundo plano (necessário para o Modo Jogo e a limpeza automática).")}>
          <Switch checked={settings.minimize_to_tray} onChange={(v) => update({ minimize_to_tray: v })} />
        </Row>
        <Row title={t("Notificações do Windows")} desc={t("Avisa quando um jogo é detectado, a limpeza termina ou há atualização.")}>
          <Switch checked={settings.notifications} onChange={(v) => update({ notifications: v })} />
        </Row>
        <Row title={t("Ponto de restauração automático")} desc={t("Cria um ponto de restauração antes de aplicar ajustes avançados (exige administrador).")}>
          <Switch checked={settings.auto_restore} onChange={(v) => update({ auto_restore: v })} />
        </Row>
        <Row title={t("Verificar atualizações ao abrir")} desc={t("Consulta o GitHub; nada é instalado sem você.")}>
          <Switch checked={settings.check_updates} onChange={(v) => update({ check_updates: v })} />
        </Row>
        <Row title={t("Limpeza automática")} desc={t("Remove temporários, relatórios de erro e miniaturas com mais de 24 horas.")}>
          <select value={settings.clean_every_days} onChange={(e) => update({ clean_every_days: Number(e.currentTarget.value) })}>
            {CLEAN_OPTIONS.map((d) => (
              <option key={d} value={d}>
                {d === 0 ? t("Desligada") : t("A cada {n} dias", { n: d })}
              </option>
            ))}
          </select>
        </Row>
        <Row title={t("Atalhos de teclado")} desc={t("Funcionam com o app aberto ou na bandeja.")}>
          <span className="keys">
            <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>G</kbd> {t("Modo Jogo")} · <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>R</kbd> {t("Liberar RAM")}
          </span>
        </Row>
      </section>

      <section className="card">
        <h3>{t("Atualizações")}</h3>
        <p>
          {t("Versão instalada")}: <b>{version || "…"}</b>
          {upd && (
            <>
              {" · "}
              {upd.newer ? (
                <b className="accent">{t("Nova versão: {v}", { v: upd.latest })}</b>
              ) : (
                <span className="ok-text">{t("Você está na versão mais recente.")}</span>
              )}
            </>
          )}
        </p>
        <div className="row">
          <button className="ghost" disabled={checking} onClick={checkNow}>
            <BtnLabel busy={checking}>{t("Verificar agora")}</BtnLabel>
          </button>
          {upd?.newer && (
            <button onClick={() => invoke("open_target", { key: "releases" })}>
              <Download size={15} /> {t("Baixar")}
            </button>
          )}
          <button className="ghost" onClick={() => invoke("open_target", { key: "repo" })}>
            <ExternalLink size={15} /> GitHub
          </button>
        </div>
        {updErr && <p className="warn">{updErr}</p>}
      </section>

      <section className="card">
        <h3>{t("Backup da configuração")}</h3>
        <p className="muted">{t("Exporta as configurações e a lista de ajustes ativos. Para restaurar, cole o conteúdo abaixo.")}</p>
        <div className="row">
          <button className="ghost" disabled={busy !== null} onClick={doExport}>
            <BtnLabel busy={busy === "export"}>
              <Download size={15} /> {t("Exportar")}
            </BtnLabel>
          </button>
          <button disabled={busy !== null || !importText.trim()} onClick={doImport}>
            <BtnLabel busy={busy === "import"}>
              <FileUp size={15} /> {t("Importar")}
            </BtnLabel>
          </button>
        </div>
        <textarea
          className="code"
          rows={5}
          value={importText || exported}
          placeholder={t("Cole aqui o conteúdo exportado para importar…")}
          onChange={(e) => {
            setExported("");
            setImportText(e.currentTarget.value);
          }}
        />
      </section>

      <section className="card">
        <div className="row spread">
          <h3>{t("Histórico de atividades")}</h3>
          <button className="ghost small" onClick={clearLog} disabled={!log?.length}>
            <Trash2 size={13} /> {t("Limpar")}
          </button>
        </div>
        <p className="muted">{t("Tudo que o Nexus mudou neste PC, do mais recente ao mais antigo.")}</p>
        {log === null && <Loading text={t("Carregando histórico…")} />}
        {log?.length === 0 && <p className="empty">{t("Nada registrado ainda.")}</p>}
        <ul className="log">
          {log?.map((e, i) => (
            <li key={i}>
              <span className={`kind k-${e.kind}`}>{e.kind}</span>
              <span>{t(e.text.split(":")[0]) + (e.text.includes(":") ? ":" + e.text.slice(e.text.indexOf(":") + 1) : "")}</span>
              <time>{new Date(e.ts * 1000).toLocaleString(settings.lang)}</time>
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}
