// Escala não linear (como nos testes de velocidade): cada marca ocupa o mesmo espaço no velocímetro.
const MARKS = [0, 5, 10, 25, 50, 100, 250, 500, 1000];
const SWEEP = 240; // graus de abertura do velocímetro
const CX = 160;
const CY = 160;
const R = 120;

export function fraction(v: number): number {
  if (v <= 0) return 0;
  const top = MARKS[MARKS.length - 1];
  if (v >= top) return 1;
  for (let i = 1; i < MARKS.length; i++) {
    if (v <= MARKS[i]) {
      const a = MARKS[i - 1];
      return (i - 1 + (v - a) / (MARKS[i] - a)) / (MARKS.length - 1);
    }
  }
  return 1;
}

const angle = (f: number) => -SWEEP / 2 + f * SWEEP; // 0° = para cima, sentido horário
const pt = (a: number, r: number) => {
  const rad = (a * Math.PI) / 180;
  return [CX + r * Math.sin(rad), CY - r * Math.cos(rad)];
};

function arc(a0: number, a1: number, r = R) {
  const [x0, y0] = pt(a0, r);
  const [x1, y1] = pt(a1, r);
  return `M${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 ${a1 - a0 > 180 ? 1 : 0} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

interface Props {
  value: number; // Mbps
  label: string;
}

export function Gauge({ value, label }: Props) {
  const f = fraction(value);
  const a = angle(f);
  const end = angle(1);
  const start = angle(0);
  return (
    <svg viewBox="0 0 320 290" className="gauge" role="img" aria-label={`${label}: ${value.toFixed(1)} Mbps`}>
      <defs>
        <linearGradient id="gv" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" style={{ stopColor: "var(--deep)" }} />
          <stop offset="1" style={{ stopColor: "var(--red-hi)" }} />
        </linearGradient>
        <filter id="gglow" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="4" result="b" />
          <feMerge>
            <feMergeNode in="b" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <path d={arc(start, end)} className="g-track" />
      {f > 0.002 && <path d={arc(start, a)} className="g-value" filter="url(#gglow)" />}
      {MARKS.map((m, i) => {
        const ma = angle(i / (MARKS.length - 1));
        const [x0, y0] = pt(ma, R - 14);
        const [x1, y1] = pt(ma, R + 2);
        const [tx, ty] = pt(ma, R - 30);
        return (
          <g key={m}>
            <line x1={x0} y1={y0} x2={x1} y2={y1} className="g-tick" />
            <text x={tx} y={ty} className="g-label" textAnchor="middle" dominantBaseline="middle">
              {m}
            </text>
          </g>
        );
      })}
      <g className="g-needle" style={{ transform: `rotate(${a}deg)`, transformOrigin: `${CX}px ${CY}px` }}>
        <line x1={CX} y1={CY + 14} x2={CX} y2={CY - R + 26} />
      </g>
      <circle cx={CX} cy={CY} r="11" className="g-hub" />
      <text x={CX} y={CY + 62} className="g-number" textAnchor="middle">
        {value >= 100 ? value.toFixed(0) : value.toFixed(1)}
      </text>
      <text x={CX} y={CY + 84} className="g-unit" textAnchor="middle">
        Mbps · {label}
      </text>
    </svg>
  );
}
