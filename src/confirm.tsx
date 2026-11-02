import { createContext, ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import { AlertTriangle, HelpCircle } from "lucide-react";
import { useT } from "./i18n";

export interface ConfirmOptions {
  title: string;
  message: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Ação destrutiva ou irreversível: ícone de alerta e foco inicial em "Cancelar". */
  danger?: boolean;
}

type Confirm = (options: ConfirmOptions) => Promise<boolean>;

const ConfirmContext = createContext<Confirm>(() => Promise.resolve(false));

/** Substitui o window.confirm: `if (!(await confirm({...}))) return;` */
export const useConfirm = () => useContext(ConfirmContext);

interface Pending extends ConfirmOptions {
  resolve: (ok: boolean) => void;
}

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const t = useT();
  const [pending, setPending] = useState<Pending | null>(null);
  const [closing, setClosing] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const okRef = useRef<HTMLButtonElement>(null);

  const confirm = useCallback<Confirm>(
    (options) =>
      new Promise<boolean>((resolve) => {
        setClosing(false);
        setPending({ ...options, resolve });
      }),
    [],
  );

  const close = useCallback(
    (ok: boolean) => {
      if (!pending) return;
      const { resolve } = pending;
      setClosing(true);
      // Espera a animação de saída antes de remover o diálogo.
      setTimeout(() => {
        setPending(null);
        setClosing(false);
        resolve(ok);
      }, 140);
    },
    [pending],
  );

  useEffect(() => {
    if (!pending) return;
    (pending.danger ? cancelRef : okRef).current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        close(false);
      } else if (e.key === "Tab") {
        // Mantém o foco dentro do diálogo.
        const order = [cancelRef.current, okRef.current].filter(Boolean) as HTMLElement[];
        const i = order.indexOf(document.activeElement as HTMLElement);
        e.preventDefault();
        order[(i + (e.shiftKey ? order.length - 1 : 1)) % order.length]?.focus();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [pending, close]);

  const Icon = pending?.danger ? AlertTriangle : HelpCircle;

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      {pending && (
        <div className={`modal-backdrop ${closing ? "closing" : ""}`} onMouseDown={() => close(false)}>
          <div
            className={`modal ${pending.danger ? "danger" : ""}`}
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="modal-title"
            aria-describedby="modal-msg"
            onMouseDown={(e) => e.stopPropagation()}
          >
            <span className="modal-icon">
              <Icon size={24} />
            </span>
            <h3 id="modal-title">{pending.title}</h3>
            <div id="modal-msg" className="modal-msg">
              {pending.message}
            </div>
            <div className="modal-actions">
              <button ref={cancelRef} className="ghost" onClick={() => close(false)}>
                {pending.cancelLabel ?? t("Cancelar")}
              </button>
              <button ref={okRef} onClick={() => close(true)}>
                {pending.confirmLabel ?? t("Confirmar")}
              </button>
            </div>
          </div>
        </div>
      )}
    </ConfirmContext.Provider>
  );
}
