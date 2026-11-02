import { LucideIcon } from "lucide-react";
import { ReactNode } from "react";

export function Spinner({ size = 16 }: { size?: number }) {
  return <span className="spinner" style={{ width: size, height: size }} aria-hidden="true" />;
}

/** Spinner + texto, para qualquer coisa que esteja carregando. */
export function Loading({ text }: { text: string }) {
  return (
    <div className="loading" role="status">
      <Spinner />
      <span>{text}</span>
    </div>
  );
}

/** Barra de progresso. Sem `value` fica indeterminada (animação contínua). */
export function ProgressBar({ value, label }: { value?: number; label?: string }) {
  const determinate = value !== undefined;
  return (
    <div className="progress" role="progressbar" aria-valuenow={determinate ? Math.round(value * 100) : undefined}>
      {label && <span className="progress-label">{label}</span>}
      <div className="progress-track">
        <div
          className={determinate ? "progress-fill" : "progress-fill indeterminate"}
          style={determinate ? { width: `${Math.max(0, Math.min(1, value)) * 100}%` } : undefined}
        />
      </div>
    </div>
  );
}

export function PageHeader({ icon: Icon, title, desc }: { icon: LucideIcon; title: string; desc?: ReactNode }) {
  return (
    <div className="page-header">
      <span className="page-icon">
        <Icon size={22} />
      </span>
      <div>
        <h2>{title}</h2>
        {desc && <p className="muted">{desc}</p>}
      </div>
    </div>
  );
}

/** Conteúdo de botão com spinner à esquerda enquanto ocupado. */
export function BtnLabel({ busy, children }: { busy?: boolean; children: ReactNode }) {
  return (
    <>
      {busy && <Spinner size={14} />}
      {children}
    </>
  );
}

/** Interruptor liga/desliga no estilo do app. */
export function Switch({ checked, onChange, disabled }: { checked: boolean; onChange: (on: boolean) => void; disabled?: boolean }) {
  return (
    <label className="switch">
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.currentTarget.checked)} />
      <i />
    </label>
  );
}
