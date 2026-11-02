import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  AppWindow,
  CalendarClock,
  Cog,
  Cpu,
  Gamepad2,
  HardDrive,
  LucideIcon,
  Monitor,
  Mouse,
  Network,
  ScrollText,
  ShieldCheck,
  SlidersHorizontal,
  Stethoscope,
  Volume2,
  Zap,
} from "lucide-react";
import { useT } from "../i18n";
import { useToast } from "../toast";
import { errText } from "../types";

interface Panel {
  key: string;
  icon: LucideIcon;
  title: string;
  sub: string;
}

// As chaves são as aceitas por open_target no backend (lista fechada).
const PANELS: Panel[] = [
  { key: "ncpa", icon: Network, title: "Conexões de rede", sub: "Adaptadores, IP e DNS" },
  { key: "powercfg", icon: Zap, title: "Opções de energia", sub: "Planos de energia" },
  { key: "sound", icon: Volume2, title: "Som", sub: "Saída, entrada e formato" },
  { key: "mouse", icon: Mouse, title: "Mouse", sub: "Botões e ponteiro" },
  { key: "joy", icon: Gamepad2, title: "Controles de jogo", sub: "Testar controle e joystick" },
  { key: "display", icon: Monitor, title: "Tela", sub: "Resolução e taxa de atualização" },
  { key: "devmgmt", icon: Cpu, title: "Gerenciador de Dispositivos", sub: "Hardware e drivers" },
  { key: "diskmgmt", icon: HardDrive, title: "Gerenciamento de Disco", sub: "Partições e unidades" },
  { key: "services", icon: Cog, title: "Serviços", sub: "Serviços do Windows" },
  { key: "taskschd", icon: CalendarClock, title: "Agendador de Tarefas", sub: "Tarefas automáticas" },
  { key: "eventvwr", icon: ScrollText, title: "Visualizador de Eventos", sub: "Erros e falhas do sistema" },
  { key: "resmon", icon: Activity, title: "Monitor de Recursos", sub: "CPU, disco e rede por processo" },
  { key: "msconfig", icon: SlidersHorizontal, title: "Configuração do Sistema", sub: "Inicialização e serviços" },
  { key: "dxdiag", icon: Stethoscope, title: "Diagnóstico do DirectX", sub: "Vídeo, som e DirectX" },
  { key: "firewall", icon: ShieldCheck, title: "Firewall", sub: "Windows Defender Firewall" },
  { key: "appwiz", icon: AppWindow, title: "Programas e recursos", sub: "Desinstalar programas" },
];

/** Atalhos para painéis clássicos do Windows, no mesmo estilo dos atalhos da tela inicial. */
export function Panels() {
  const t = useT();
  const toast = useToast();

  async function open(p: Panel) {
    try {
      await invoke("open_target", { key: p.key });
    } catch (e) {
      toast(t(errText(e)));
    }
  }

  return (
    <>
      <h3 className="section-title">{t("Painéis do Windows")}</h3>
      <div className="tiles">
        {PANELS.map((p) => {
          const Icon = p.icon;
          return (
            <button key={p.key} className="tile" onClick={() => open(p)}>
              <span className="tile-icon">
                <Icon size={22} />
              </span>
              <span className="tile-text">
                <b>{t(p.title)}</b>
                <small>{t(p.sub)}</small>
              </span>
            </button>
          );
        })}
      </div>
    </>
  );
}
