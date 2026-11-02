/** Perfis prontos: conjuntos de ajustes (ids do catálogo) para cada tipo de uso. */
export interface Preset {
  id: string;
  name: string;
  desc: string;
  tweaks: string[];
}

export const PRESETS: Preset[] = [
  {
    id: "competitivo",
    name: "Competitivo",
    desc: "Menor latência para jogos online: desliga captura em segundo plano, notificações e economia de energia.",
    tweaks: [
      "game-mode", "gamedvr-off", "gamebar-popups-off", "fso-off", "windowed-optimizations", "power-ultimate",
      "mouse-accel-off", "sticky-keys-off", "toasts-off", "background-apps-off", "delivery-opt-off",
      "transparency-off", "visual-perf", "edge-bg-off",
    ],
  },
  {
    id: "equilibrado",
    name: "Equilibrado",
    desc: "Só o que é seguro e útil no dia a dia: menos anúncios, menos processos em segundo plano.",
    tweaks: [
      "game-mode", "gamedvr-off", "mouse-accel-off", "sticky-keys-off", "delivery-opt-off", "tips-ads-off",
      "ad-id-off", "startup-delay-off", "file-ext-visible", "edge-bg-off", "widgets-off",
    ],
  },
  {
    id: "streamer",
    name: "Streamer",
    desc: "Mantém a captura do Windows e silencia interrupções durante a transmissão.",
    tweaks: [
      "game-mode", "toasts-off", "sticky-keys-off", "mouse-accel-off", "delivery-opt-off", "edge-bg-off", "power-ultimate",
    ],
  },
  {
    id: "notebook",
    name: "Notebook na bateria",
    desc: "Reduz processos em segundo plano sem mexer no plano de energia, para não gastar bateria.",
    tweaks: [
      "transparency-off", "visual-perf", "background-apps-off", "delivery-opt-off", "edge-bg-off", "widgets-off",
      "tips-ads-off", "telemetry-min", "startup-delay-off",
    ],
  },
];

/** Sugere um perfil a partir do hardware (resultado de system_info). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function suggestPreset(info: any): string {
  const arr = (v: unknown): unknown[] => (v == null ? [] : Array.isArray(v) ? v : [v]);
  if (info?.battery) return "notebook";
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const threads = (arr(info?.cpu)[0] as any)?.NumberOfLogicalProcessors ?? 0;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const dedicated = arr(info?.gpus).some((g: any) => /nvidia|geforce|radeon rx|arc/i.test(String(g?.Name)));
  return dedicated && threads >= 8 ? "competitivo" : "equilibrado";
}
