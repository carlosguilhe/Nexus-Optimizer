import { Tweak } from "../types";
import { useT } from "../i18n";
import { tabIcon } from "../icons";
import { BtnLabel } from "./ui";

interface Props {
  tweaks: Tweak[];
  admin: boolean;
  busy: string | null;
  onToggle: (t: Tweak) => void;
  showCategory?: boolean;
}

export function TweakList({ tweaks, admin, busy, onToggle, showCategory }: Props) {
  const t = useT();
  if (!tweaks.length) return null;
  return (
    <ul className="tweaks">
      {tweaks.map((tw) => {
        const Icon = tabIcon(tw.category);
        return (
          <li key={tw.id} className={tw.applied ? "on" : ""}>
            <div className="info">
              <div className="title">
                <strong>{t(tw.name)}</strong>
                <span className={`chip risk-${tw.risk}`}>{t(tw.risk)}</span>
                {showCategory && (
                  <span className="chip cat">
                    <Icon size={12} />
                    {t(tw.category)}
                  </span>
                )}
                {tw.admin && <span className="chip">admin</span>}
                {tw.reboot && <span className="chip">{t("reiniciar")}</span>}
              </div>
              <p>{t(tw.description)}</p>
              <small>{t(tw.evidence)}</small>
            </div>
            <button
              className={tw.applied ? "ghost" : ""}
              disabled={busy !== null || (tw.admin && !admin)}
              onClick={() => onToggle(tw)}
            >
              <BtnLabel busy={busy === tw.id}>{tw.applied ? t("Reverter") : t("Aplicar")}</BtnLabel>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
