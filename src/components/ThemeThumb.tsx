import { Theme, themeStyle } from "../theme";

/** Miniatura que mostra como o app ficaria com o tema: barra lateral, atalhos, botão e barra de progresso. */
export function ThemeThumb({ theme }: { theme: Theme }) {
  return (
    <div className="thumb" style={themeStyle(theme)} aria-hidden="true">
      <div className="thumb-side">
        <span className="thumb-logo" />
        <span className="thumb-nav active" />
        <span className="thumb-nav" />
        <span className="thumb-nav" />
        <span className="thumb-nav" />
      </div>
      <div className="thumb-main">
        <div className="thumb-top">
          <span className="thumb-search" />
          <span className="thumb-pill" />
        </div>
        <span className="thumb-title" />
        <div className="thumb-tiles">
          <span className="thumb-tile">
            <i />
          </span>
          <span className="thumb-tile">
            <i />
          </span>
          <span className="thumb-tile on">
            <i />
          </span>
        </div>
        <div className="thumb-bar">
          <span />
        </div>
        <span className="thumb-btn" />
      </div>
    </div>
  );
}
