import { createContext, ReactNode, useCallback, useContext, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme } from "./theme";
import { Settings } from "./types";

const FALLBACK: Settings = {
  auto_game_mode: false,
  auto_restore: true,
  minimize_to_tray: false,
  notifications: true,
  clean_every_days: 0,
  last_clean: 0,
  check_updates: true,
  theme: "nexus",
  lang: "pt-BR",
  wizard_done: true, // até carregar de verdade, não mostra o assistente
  default_profile: { priority: "High", close_apps: [], tweaks: [], affinity: null },
  profiles: [],
};

interface Ctx {
  settings: Settings;
  loaded: boolean;
  /** Altera parte das configurações e grava no disco. */
  update: (patch: Partial<Settings>) => Promise<void>;
}

const SettingsContext = createContext<Ctx>({ settings: FALLBACK, loaded: false, update: async () => {} });

export const useSettings = () => useContext(SettingsContext);

function cached(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  // Tema e idioma do último uso evitam um flash de cores/idioma errados enquanto o app carrega.
  const [settings, setSettings] = useState<Settings>({
    ...FALLBACK,
    theme: cached("nexus.theme", "nexus"),
    lang: cached("nexus.lang", "pt-BR"),
  });
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    invoke<Settings>("settings_get")
      .then((s) => {
        setSettings(s);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
    const un = listen<Settings>("settings", (e) => setSettings(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    applyTheme(settings.theme);
    try {
      localStorage.setItem("nexus.theme", settings.theme);
      localStorage.setItem("nexus.lang", settings.lang);
    } catch {
      /* sem armazenamento: só não lembra */
    }
    document.documentElement.lang = settings.lang;
  }, [settings.theme, settings.lang]);

  const update = useCallback(
    async (patch: Partial<Settings>) => {
      const next = { ...settings, ...patch };
      setSettings(next);
      try {
        setSettings(await invoke<Settings>("settings_set", { value: next }));
      } catch {
        setSettings(settings); // não gravou: volta ao valor anterior
      }
    },
    [settings],
  );

  return <SettingsContext.Provider value={{ settings, loaded, update }}>{children}</SettingsContext.Provider>;
}
