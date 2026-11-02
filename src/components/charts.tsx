const W = 600;

interface Series {
  values: number[];
  color: string;
}

interface Guide {
  y: number;
  label: string;
}

/** Gráfico de linhas simples (frametime, ping). Eixo Y de 0 até `max`; `guides` desenha linhas de referência. */
export function LineChart({
  series,
  max,
  height = 150,
  guides = [],
  unit = "ms",
}: {
  series: Series[];
  max: number;
  height?: number;
  guides?: Guide[];
  unit?: string;
}) {
  const H = height;
  const pad = 14;
  const y = (v: number) => H - pad - (Math.min(v, max) / max) * (H - pad * 2);
  return (
    <svg className="chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img">
      {guides.filter((g) => g.y < max).map((g) => (
        <g key={g.label}>
          <line x1="0" x2={W} y1={y(g.y)} y2={y(g.y)} className="chart-guide" />
          <text x="4" y={y(g.y) - 3} className="chart-label">
            {g.label}
          </text>
        </g>
      ))}
      {series.map((s, i) => {
        if (s.values.length < 2) return null;
        const pts = s.values.map((v, k) => `${((k / (s.values.length - 1)) * W).toFixed(1)},${y(v).toFixed(1)}`);
        return <polyline key={i} points={pts.join(" ")} fill="none" stroke={s.color} strokeWidth="1.6" vectorEffect="non-scaling-stroke" />;
      })}
      <text x={W - 4} y="11" textAnchor="end" className="chart-label">
        {max} {unit}
      </text>
    </svg>
  );
}

/** Mini gráfico de área para o monitor ao vivo. */
export function Sparkline({ values, max, color = "var(--red-hi)" }: { values: number[]; max: number; color?: string }) {
  const H = 46;
  if (values.length < 2) return <svg className="spark" viewBox={`0 0 ${W} ${H}`} />;
  const pts = values.map((v, k) => `${((k / (values.length - 1)) * W).toFixed(1)},${(H - 3 - (Math.min(v, max) / max) * (H - 6)).toFixed(1)}`);
  return (
    <svg className="spark" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none">
      <polygon points={`0,${H} ${pts.join(" ")} ${W},${H}`} fill={color} opacity="0.16" />
      <polyline points={pts.join(" ")} fill="none" stroke={color} strokeWidth="1.8" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

export const CHART_COLORS = ["var(--red-hi)", "#4ea1ff", "#7dd87d", "#f5b301"];
