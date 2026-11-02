import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { fmtBytes } from "../types";

interface Usage {
  cpu_pct: number;
  ram_used: number;
  ram_total: number;
}

export function UsageBars({ intervalMs = 2000 }: { intervalMs?: number }) {
  const [u, setU] = useState<Usage | null>(null);

  useEffect(() => {
    let alive = true;
    const tick = () =>
      invoke<Usage>("usage")
        .then((r) => alive && setU(r))
        .catch(() => {});
    tick();
    const id = setInterval(tick, intervalMs);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [intervalMs]);

  const cpu = u?.cpu_pct ?? 0;
  const ram = u && u.ram_total ? (u.ram_used / u.ram_total) * 100 : 0;
  return (
    <div className="usage">
      <div>
        <span>CPU</span>
        <div className="bar"><i style={{ width: `${Math.min(cpu, 100)}%` }} /></div>
        <b>{cpu.toFixed(0)}%</b>
      </div>
      <div>
        <span>RAM</span>
        <div className="bar"><i style={{ width: `${Math.min(ram, 100)}%` }} /></div>
        <b>{u ? `${fmtBytes(u.ram_used)} / ${fmtBytes(u.ram_total)}` : "—"}</b>
      </div>
    </div>
  );
}
