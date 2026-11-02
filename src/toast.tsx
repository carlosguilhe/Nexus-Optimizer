import { createContext, ReactNode, useCallback, useContext, useRef, useState } from "react";

type Notify = (message: string) => void;

const ToastContext = createContext<Notify>(() => {});

export const useToast = () => useContext(ToastContext);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [msg, setMsg] = useState("");
  const timer = useRef<number | undefined>(undefined);

  const notify = useCallback((m: string) => {
    setMsg(m);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setMsg(""), 3200);
  }, []);

  return (
    <ToastContext.Provider value={notify}>
      {children}
      {msg && <div className="toast" role="status">{msg}</div>}
    </ToastContext.Provider>
  );
}
