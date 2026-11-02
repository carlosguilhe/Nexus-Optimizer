export interface Tweak {
  id: string;
  category: string;
  admin: boolean;
  reboot: boolean;
  name: string;
  description: string;
  risk: string;
  evidence: string;
  applied: boolean;
}

export interface Bench {
  frames: number;
  avg_fps: number;
  low_1pct_fps: number;
  max_frametime_ms: number;
}

export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function errText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

export interface Profile {
  priority: string;
  close_apps: string[];
  tweaks: string[];
  affinity: number | null;
}

export interface GameProfile {
  exe: string;
  name: string;
  profile: Profile;
}

export interface Settings {
  auto_game_mode: boolean;
  auto_restore: boolean;
  minimize_to_tray: boolean;
  notifications: boolean;
  clean_every_days: number;
  last_clean: number;
  check_updates: boolean;
  auto_update: boolean;
  theme: string;
  lang: string;
  wizard_done: boolean;
  default_profile: Profile;
  profiles: GameProfile[];
}

export interface BenchResult {
  frames: number;
  avg_fps: number;
  low_1pct_fps: number;
  max_frametime_ms: number;
  series: number[];
}

export interface HistoryEntry {
  id: number;
  ts: number;
  game: string;
  process: string;
  label: string;
  tweaks: string[];
  result: BenchResult;
}
