import { useState } from "react";

/** Ícone do jogo; se não houver imagem (ou a internet falhar), mostra a inicial no estilo da marca. */
export function GameIcon({ url, name, size = 44, className = "" }: { url: string | null; name: string; size?: number; className?: string }) {
  const [failed, setFailed] = useState(false);
  const style = { width: size, height: size };
  if (!url || failed) {
    return (
      <span className={`game-icon fallback ${className}`} style={style}>
        {name.trim().charAt(0).toUpperCase() || "?"}
      </span>
    );
  }
  return (
    <img
      className={`game-icon ${className}`}
      style={style}
      src={url}
      alt=""
      loading="lazy"
      referrerPolicy="no-referrer"
      onError={() => setFailed(true)}
    />
  );
}
