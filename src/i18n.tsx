import { createContext, ReactNode, useCallback, useContext } from "react";
import { useSettings } from "./settings";
import { en } from "./locales/en";
import { es } from "./locales/es";

export const LANGUAGES = [
  { id: "pt-BR", label: "Português (Brasil)" },
  { id: "en", label: "English" },
  { id: "es", label: "Español" },
] as const;

type Vars = Record<string, string | number>;
export type T = (key: string, vars?: Vars) => string;

const DICTS: Record<string, Record<string, string>> = { en, es };

function fill(text: string, vars?: Vars): string {
  return vars ? text.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m)) : text;
}

/** O texto original está em português (pt-BR); as outras línguas são buscadas pelo texto em português. */
export function translate(lang: string, key: string, vars?: Vars): string {
  const text = lang === "pt-BR" ? key : (DICTS[lang]?.[key] ?? key);
  return fill(text, vars);
}

const I18nContext = createContext<T>((k, v) => fill(k, v));

export const useT = () => useContext(I18nContext);

export function I18nProvider({ children }: { children: ReactNode }) {
  const { settings } = useSettings();
  const lang = settings.lang;
  const t = useCallback<T>((key, vars) => translate(lang, key, vars), [lang]);
  return <I18nContext.Provider value={t}>{children}</I18nContext.Provider>;
}
