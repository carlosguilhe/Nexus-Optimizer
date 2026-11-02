import { ProgressBar } from "./ui";

interface Props {
  progress: number;
  label: string;
  leaving: boolean;
}

/** Tela de abertura: logo desenhado com animação enquanto o app carrega os dados iniciais. */
export function Splash({ progress, label, leaving }: Props) {
  return (
    <div className={`splash ${leaving ? "leaving" : ""}`} aria-hidden={leaving}>
      <div className="splash-glow" />
      <svg className="splash-logo" viewBox="0 0 400 400" width="190" height="190">
        <defs>
          <linearGradient id="sp-red" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" style={{ stopColor: "var(--red-hi)" }} />
            <stop offset="0.55" style={{ stopColor: "var(--red)" }} />
            <stop offset="1" style={{ stopColor: "var(--deep)" }} />
          </linearGradient>
          <filter id="sp-glow" x="-40%" y="-40%" width="180%" height="180%">
            <feGaussianBlur stdDeviation="9" result="b" />
            <feMerge>
              <feMergeNode in="b" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <path className="sp-ring-bg" d="M80 285 A150 150 0 1 1 320 285" />
        <path className="sp-ring" d="M80 285 A150 150 0 1 1 320 285" filter="url(#sp-glow)" />
        <path
          className="sp-n"
          filter="url(#sp-glow)"
          fill="url(#sp-red)"
          d="M118 296 V104 H158 L246 220 V104 H286 V296 H246 L158 180 V296 Z"
        />
      </svg>
      <div className="splash-title">
        <b>NEXUS</b>
        <small>OPTIMIZER</small>
      </div>
      <div className="splash-progress">
        <ProgressBar value={progress} />
        <span>{label}</span>
      </div>
    </div>
  );
}
