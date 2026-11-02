import { CSSProperties } from "react";

/** Todas as cores do app vêm destas variáveis CSS. Cada tema define o conjunto completo. */
export type ThemeVars = Record<string, string>;

export interface Theme {
  id: string;
  /** Nome em português (também é a chave de tradução). */
  name: string;
  light?: boolean;
  vars: ThemeVars;
}

const SEMANTIC_DARK: ThemeVars = {
  "--on-tint": "#ffffff",
  "--warn-bg": "#2a1d0b",
  "--warn-line": "#b3541e",
  "--warn-fg": "#ffb27a",
  "--ok-bg": "#101a10",
  "--ok-line": "#2c5a2c",
  "--ok-fg": "#9fe29f",
  "--err-fg": "#ff8d97",
};

/** Tema escuro gerado a partir de um matiz (0-360). */
function dark(h: number, s = 1): ThemeVars {
  const hsl = (sat: number, l: number) => `hsl(${h} ${Math.round(sat * s)}% ${l}%)`;
  return {
    "--bg": hsl(18, 3),
    "--panel": hsl(10, 5.5),
    "--card": hsl(8, 8),
    "--inset": hsl(10, 4.5),
    "--line": hsl(40, 13),
    "--line2": "#333333",
    "--track": hsl(35, 10),
    "--red": hsl(80, 50),
    "--red-hi": hsl(100, 62),
    "--red-dim": hsl(55, 15),
    "--deep": hsl(85, 29),
    "--tint": hsl(60, 9),
    "--tint2": hsl(50, 6.5),
    "--text": "#f1f1f1",
    "--text2": "#bdbdbd",
    "--muted": "#8d8d8d",
    "--faint": "#5d5d5d",
    ...SEMANTIC_DARK,
  };
}

const NEXUS: ThemeVars = {
  "--bg": "#070707",
  "--panel": "#0e0e0e",
  "--card": "#141414",
  "--inset": "#0b0b0b",
  "--line": "#2a1417",
  "--line2": "#333333",
  "--track": "#1f1214",
  "--red": "#e5202e",
  "--red-hi": "#ff3b4a",
  "--red-dim": "#3a1216",
  "--deep": "#8c0b16",
  "--tint": "#2a0a0e",
  "--tint2": "#1a0a0d",
  "--text": "#f1f1f1",
  "--text2": "#bdbdbd",
  "--muted": "#8d8d8d",
  "--faint": "#5d5d5d",
  ...SEMANTIC_DARK,
};

const GRAFITE: ThemeVars = {
  ...dark(0, 0),
  "--red": "#d9d9de",
  "--red-hi": "#ffffff",
  "--red-dim": "#2c2c31",
  "--deep": "#8a8a92",
  "--tint": "#202024",
  "--tint2": "#17171a",
  "--line": "#2a2a2f",
  "--track": "#232327",
};

const CLARO: ThemeVars = {
  "--bg": "#f3f3f6",
  "--panel": "#ffffff",
  "--card": "#ffffff",
  "--inset": "#f1f1f5",
  "--line": "#e6d6d8",
  "--line2": "#cfcfd8",
  "--track": "#ece6e7",
  "--red": "#d41f2c",
  "--red-hi": "#e5202e",
  "--red-dim": "#fbd9dc",
  "--deep": "#a8121d",
  "--tint": "#fdeced",
  "--tint2": "#fff5f6",
  "--text": "#17171a",
  "--text2": "#3a3a44",
  "--muted": "#6b6b78",
  "--faint": "#9a9aa6",
  "--on-tint": "#3a0a10",
  "--warn-bg": "#fff3e3",
  "--warn-line": "#e0903a",
  "--warn-fg": "#8a4a0a",
  "--ok-bg": "#e8f6e8",
  "--ok-line": "#6bb86b",
  "--ok-fg": "#1d6b1d",
  "--err-fg": "#b3121e",
};

export const THEMES: Theme[] = [
  { id: "nexus", name: "Nexus Vermelho", vars: NEXUS },
  { id: "ciano", name: "Ciano Neon", vars: dark(187) },
  { id: "matrix", name: "Verde Matrix", vars: dark(142) },
  { id: "royal", name: "Violeta Royal", vars: dark(265) },
  { id: "sunset", name: "Laranja Sunset", vars: dark(24) },
  { id: "oceano", name: "Azul Oceano", vars: dark(215) },
  { id: "ouro", name: "Dourado", vars: dark(43) },
  { id: "sakura", name: "Rosa Sakura", vars: dark(330) },
  { id: "grafite", name: "Grafite", vars: GRAFITE },
  { id: "claro", name: "Claro", light: true, vars: CLARO },
];

export const themeById = (id: string): Theme => THEMES.find((t) => t.id === id) ?? THEMES[0];

/** Aplica o tema no app inteiro. */
export function applyTheme(id: string) {
  const t = themeById(id);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(t.vars)) root.style.setProperty(k, v);
  root.style.setProperty("color-scheme", t.light ? "light" : "dark");
  root.dataset.theme = t.id;
}

/** Variáveis como estilo inline, para desenhar a miniatura de um tema sem aplicá-lo. */
export const themeStyle = (t: Theme): CSSProperties => t.vars as unknown as CSSProperties;
