import {
  Activity,
  Cpu,
  FlaskConical,
  Gamepad2,
  Gauge,
  Globe,
  HardDrive,
  LayoutDashboard,
  LucideIcon,
  MousePointer2,
  Package,
  Palette,
  Rocket,
  Settings2,
  ShieldCheck,
  Timer,
  Wrench,
  Zap,
  Settings,
  LifeBuoy,
} from "lucide-react";

/** Ícone de cada aba/categoria (as categorias do catálogo têm o mesmo nome das abas). */
export const TAB_ICONS: Record<string, LucideIcon> = {
  Início: LayoutDashboard,
  Jogos: Gamepad2,
  Energia: Zap,
  Entrada: MousePointer2,
  Rede: Globe,
  Velocidade: Gauge,
  Visual: Palette,
  Privacidade: ShieldCheck,
  Serviços: Settings2,
  Sistema: HardDrive,
  Aplicativos: Package,
  Inicialização: Rocket,
  Processos: Activity,
  Avançado: FlaskConical,
  Benchmark: Timer,
  Monitor: Cpu,
  Manutenção: Wrench,
  Configurações: Settings,
  Ajuda: LifeBuoy,
};

export const tabIcon = (name: string): LucideIcon => TAB_ICONS[name] ?? Cpu;

/** Abas agrupadas na barra lateral. */
export const GROUPS: { label: string | null; tabs: string[] }[] = [
  { label: null, tabs: ["Início"] },
  { label: "Otimização", tabs: ["Jogos", "Energia", "Entrada", "Rede", "Visual", "Privacidade", "Serviços", "Sistema", "Avançado"] },
  { label: "Ferramentas", tabs: ["Velocidade", "Benchmark", "Monitor", "Processos", "Inicialização", "Aplicativos", "Manutenção"] },
  { label: "Nexus", tabs: ["Configurações", "Ajuda"] },
];

export const TABS = GROUPS.flatMap((g) => g.tabs);
