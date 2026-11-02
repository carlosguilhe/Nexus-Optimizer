import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Trash2, X } from "lucide-react";
import { errText, GameProfile, Profile, Tweak } from "../types";
import { useT } from "../i18n";
import { useSettings } from "../settings";
import { useToast } from "../toast";
import { useConfirm } from "../confirm";
import { BtnLabel } from "./ui";
import { GameIcon } from "./GameIcon";

const PRIORITIES = [
  ["High", "Alta"],
  ["AboveNormal", "Acima do normal"],
  ["Normal", "Normal"],
] as const;

/** Editor de um perfil: prioridade, apps a fechar, ajustes temporários e núcleos de CPU. */
function ProfileEditor({ profile, onChange, tweaks }: { profile: Profile; onChange: (p: Profile) => void; tweaks: Tweak[] }) {
  const t = useT();
  const [cores, setCores] = useState(0);
  const [app, setApp] = useState("");
  const [open, setOpen] = useState(false);

  useEffect(() => {
    invoke<number>("cpu_count").then(setCores);
  }, []);

  const mask = profile.affinity;
  const coreOn = (i: number) => mask == null || (BigInt(mask) >> BigInt(i)) & 1n ? true : false;
  const toggleCore = (i: number) => {
    const all = (1n << BigInt(cores)) - 1n;
    const cur = mask == null ? all : BigInt(mask);
    const next = cur ^ (1n << BigInt(i));
    onChange({ ...profile, affinity: next === all || next === 0n ? null : Number(next) });
  };
  const addApp = () => {
    const n = app.trim();
    if (n && !profile.close_apps.some((a) => a.toLowerCase() === n.toLowerCase())) {
      onChange({ ...profile, close_apps: [...profile.close_apps, n.toLowerCase().endsWith(".exe") ? n : `${n}.exe`] });
    }
    setApp("");
  };
  const toggleTweak = (id: string) =>
    onChange({
      ...profile,
      tweaks: profile.tweaks.includes(id) ? profile.tweaks.filter((x) => x !== id) : [...profile.tweaks, id],
    });

  return (
    <div className="profile-editor">
      <label className="field">
        <span>{t("Prioridade do jogo")}</span>
        <select value={profile.priority} onChange={(e) => onChange({ ...profile, priority: e.currentTarget.value })}>
          {PRIORITIES.map(([v, l]) => (
            <option key={v} value={v}>{t(l)}</option>
          ))}
        </select>
      </label>

      <div className="field">
        <span>{t("Fechar ao iniciar o jogo")}</span>
        <div className="chips">
          {profile.close_apps.map((a) => (
            <span key={a} className="chip-del">
              {a}
              <button aria-label={t("Remover")} onClick={() => onChange({ ...profile, close_apps: profile.close_apps.filter((x) => x !== a) })}>
                <X size={12} />
              </button>
            </span>
          ))}
        </div>
        <div className="row">
          <input value={app} onChange={(e) => setApp(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && addApp()} placeholder="chrome.exe" />
          <button className="ghost" onClick={addApp} disabled={!app.trim()}>
            <Plus size={14} /> {t("Adicionar")}
          </button>
        </div>
        <small className="muted">{t("Dados não salvos nesses programas serão perdidos. Processos do Windows nunca são fechados.")}</small>
      </div>

      <div className="field">
        <span>{t("Ajustes ativos só durante o jogo")}</span>
        <button className="ghost small" onClick={() => setOpen(!open)}>
          {open ? t("Ocultar lista") : t("Escolher ajustes ({n})", { n: profile.tweaks.length })}
        </button>
        {open && (
          <ul className="checks compact">
            {tweaks
              .filter((x) => !x.reboot)
              .map((x) => (
                <li key={x.id}>
                  <label>
                    <input type="checkbox" checked={profile.tweaks.includes(x.id)} onChange={() => toggleTweak(x.id)} />
                    <span>
                      <b>{t(x.name)}</b>
                      {x.admin && <span className="chip">admin</span>}
                    </span>
                  </label>
                </li>
              ))}
          </ul>
        )}
        <small className="muted">{t("Quando o jogo fecha, o Nexus desfaz o que ele mesmo aplicou. Ajustes que exigem reiniciar não aparecem aqui.")}</small>
      </div>

      <div className="field">
        <span>{t("Núcleos de CPU permitidos")}</span>
        <div className="cores">
          {Array.from({ length: cores }, (_, i) => (
            <button key={i} className={coreOn(i) ? "core on" : "core"} onClick={() => toggleCore(i)}>
              {i}
            </button>
          ))}
        </div>
        <small className="muted">{t("Todos ligados = sem restrição (recomendado). Limitar núcleos só ajuda em casos específicos; meça antes.")}</small>
      </div>
    </div>
  );
}

interface Hit {
  name: string;
  process: string;
  icon_url: string | null;
}

export function ProfilesCard({ tweaks }: { tweaks: Tweak[] }) {
  const t = useT();
  const confirm = useConfirm();
  const { settings, update } = useSettings();
  const [selected, setSelected] = useState<string>("*");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [running, setRunning] = useState<Hit[]>([]);

  useEffect(() => {
    invoke<{ process: string; name: string; icon_url: string | null }[]>("running_games").then((g) =>
      setRunning(g.map((x) => ({ name: x.name, process: x.process, icon_url: x.icon_url }))),
    );
  }, []);

  useEffect(() => {
    if (query.trim().length < 2) return setHits([]);
    const id = setTimeout(() => invoke<Hit[]>("search_games", { query }).then(setHits).catch(() => setHits([])), 250);
    return () => clearTimeout(id);
  }, [query]);

  const current: Profile = selected === "*" ? settings.default_profile : (settings.profiles.find((p) => p.exe === selected)?.profile ?? settings.default_profile);

  function save(p: Profile) {
    if (selected === "*") update({ default_profile: p });
    else update({ profiles: settings.profiles.map((g) => (g.exe === selected ? { ...g, profile: p } : g)) });
  }

  function add(h: Hit) {
    if (!settings.profiles.some((p) => p.exe === h.process)) {
      const gp: GameProfile = { exe: h.process, name: h.name, profile: { ...settings.default_profile } };
      update({ profiles: [...settings.profiles, gp] });
    }
    setSelected(h.process);
    setQuery("");
  }

  async function remove(exe: string) {
    const g = settings.profiles.find((p) => p.exe === exe);
    const ok = await confirm({
      title: t("Remover o perfil de {n}?", { n: g?.name ?? exe }),
      message: t("O jogo volta a usar o perfil padrão."),
      confirmLabel: t("Remover"),
      danger: true,
    });
    if (!ok) return;
    setSelected("*");
    update({ profiles: settings.profiles.filter((p) => p.exe !== exe) });
  }

  const suggestions = (query.trim().length >= 2 ? hits : running).filter((h) => !settings.profiles.some((p) => p.exe === h.process));

  return (
    <section className="card">
      <h3>{t("Perfis por jogo")}</h3>
      <p className="muted">
        {t("Cada jogo pode ter o seu perfil. Com o Modo Jogo automático ligado, o Nexus aplica o perfil quando o jogo abre e desfaz tudo quando ele fecha. Jogos sem perfil próprio usam o perfil padrão.")}
      </p>
      <div className="profile-tabs">
        <button className={selected === "*" ? "tab on" : "tab"} onClick={() => setSelected("*")}>
          {t("Perfil padrão")}
        </button>
        {settings.profiles.map((g) => (
          <button key={g.exe} className={selected === g.exe ? "tab on" : "tab"} onClick={() => setSelected(g.exe)}>
            {g.name}
          </button>
        ))}
      </div>

      <ProfileEditor profile={current} onChange={save} tweaks={tweaks} />
      {selected !== "*" && (
        <div className="row">
          <button className="ghost" onClick={() => remove(selected)}>
            <Trash2 size={14} /> {t("Remover este perfil")}
          </button>
        </div>
      )}

      <h4 className="subhead">{t("Adicionar jogo")}</h4>
      <input className="search-plain" value={query} onChange={(e) => setQuery(e.currentTarget.value)} placeholder={t("Buscar jogo no catálogo (ex.: valorant)…")} />
      {query.trim().length < 2 && running.length > 0 && <small className="muted">{t("Abertos agora:")}</small>}
      <ul className="game-list">
        {suggestions.slice(0, 8).map((h) => (
          <li key={h.process}>
            <button className="game" onClick={() => add(h)}>
              <GameIcon url={h.icon_url} name={h.name} />
              <span className="game-text">
                <b>{h.name}</b>
                <small>{h.process}</small>
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function AffinityTool() {
  const t = useT();
  const toast = useToast();
  const [proc, setProc] = useState("");
  const [cores, setCores] = useState(0);
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<number>("cpu_count").then((n) => {
      setCores(n);
      setSel(new Set(Array.from({ length: n }, (_, i) => i)));
    });
  }, []);

  async function apply() {
    const mask = [...sel].reduce((m, i) => m | (1n << BigInt(i)), 0n);
    setBusy(true);
    try {
      const n = await invoke<number>("set_affinity_by_name", { process: proc, mask: Number(mask) });
      toast(t("Núcleos definidos em {n} processo(s). Volta ao normal quando o programa fechar.", { n }));
    } catch (e) {
      toast(t(errText(e)));
    }
    setBusy(false);
  }

  return (
    <section className="card">
      <h3>{t("Afinidade de CPU")}</h3>
      <p className="muted">{t("Escolha em quais núcleos um programa aberto pode rodar. Não é permanente. Só ajuda em casos específicos (por exemplo, evitar núcleos ocupados); meça com o benchmark.")}</p>
      <div className="row">
        <input value={proc} onChange={(e) => setProc(e.currentTarget.value)} placeholder="ex.: VALORANT-Win64-Shipping.exe" />
        <button disabled={busy || !proc || sel.size === 0} onClick={apply}>
          <BtnLabel busy={busy}>{t("Aplicar")}</BtnLabel>
        </button>
      </div>
      <div className="cores">
        {Array.from({ length: cores }, (_, i) => (
          <button
            key={i}
            className={sel.has(i) ? "core on" : "core"}
            onClick={() =>
              setSel((s) => {
                const n = new Set(s);
                if (n.has(i)) n.delete(i);
                else n.add(i);
                return n;
              })
            }
          >
            {i}
          </button>
        ))}
      </div>
    </section>
  );
}

interface Msi {
  id: string;
  name: string;
  class: string;
  msi: boolean | null;
  changed: boolean;
}

export function MsiCard({ admin }: { admin: boolean }) {
  const t = useT();
  const toast = useToast();
  const confirm = useConfirm();
  const [list, setList] = useState<Msi[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = () => invoke<Msi[]>("msi_list").then(setList).catch(() => setList([]));
  useEffect(() => {
    load();
  }, []);

  async function toggle(d: Msi) {
    const enable = !d.changed;
    if (enable) {
      const ok = await confirm({
        title: t("Ativar MSI Mode em {n}?", { n: d.name }),
        message: t("Muda como o dispositivo envia interrupções ao processador. Se algo ficar instável, use Reverter e reinicie. Exige reiniciar o PC."),
        confirmLabel: t("Ativar"),
        danger: true,
      });
      if (!ok) return;
    }
    setBusy(d.id);
    try {
      await invoke("msi_set", { id: d.id, enabled: enable });
      toast(enable ? t("MSI Mode ativado. Reinicie o PC.") : t("MSI Mode revertido. Reinicie o PC."));
    } catch (e) {
      toast(t(errText(e)));
    }
    await load();
    setBusy(null);
  }

  return (
    <section className="card">
      <h3>MSI Mode</h3>
      <p className="muted">
        {t("Interrupções sinalizadas por mensagem (MSI) costumam reduzir a latência de placas de vídeo e de rede. Muitas GPUs modernas já vêm com ele ativo; se aparecer \"ativo\", não há o que fazer.")}
      </p>
      {list === null && <p className="muted">…</p>}
      <ul className="drivers">
        {list?.map((d) => (
          <li key={d.id}>
            <div>
              <b>{d.name}</b>
              <small>
                {d.class} · MSI: {d.msi == null ? t("não definido") : d.msi ? t("ativo") : t("desativado")}
                {d.changed && <span className="chip cat">{t("alterado pelo Nexus")}</span>}
              </small>
            </div>
            <button className={d.changed ? "ghost small" : "small"} disabled={!admin || busy !== null || (d.msi === true && !d.changed)} onClick={() => toggle(d)}>
              <BtnLabel busy={busy === d.id}>{d.changed ? t("Reverter") : t("Ativar")}</BtnLabel>
            </button>
          </li>
        ))}
      </ul>
      {!admin && <p className="warn">{t("Exige administrador.")}</p>}
    </section>
  );
}
