import type { ThemeMode } from "../hooks/useTheme";
import LogoMark from "./LogoMark";

interface Props {
  mode: ThemeMode;
  onMode: (m: ThemeMode) => void;
}

const OPTIONS: { key: ThemeMode; label: string; sub: string; icon: React.ReactNode }[] = [
  {
    key: "system",
    label: "System",
    sub: "match macOS",
    icon: (
      <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
        <circle cx="9" cy="9" r="6.4" stroke="currentColor" strokeWidth="1.4" />
        <path d="M9 2.6a6.4 6.4 0 0 1 0 12.8Z" fill="currentColor" opacity=".55" />
      </svg>
    ),
  },
  {
    key: "light",
    label: "Light",
    sub: "always light",
    icon: (
      <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
        <circle cx="9" cy="9" r="3.4" stroke="currentColor" strokeWidth="1.4" />
        <path
          d="M9 1.6v2M9 14.4v2M1.6 9h2M14.4 9h2M3.8 3.8l1.4 1.4M12.8 12.8l1.4 1.4M14.2 3.8l-1.4 1.4M5.2 12.8l-1.4 1.4"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
        />
      </svg>
    ),
  },
  {
    key: "dark",
    label: "Dark",
    sub: "always dark",
    icon: (
      <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
        <path
          d="M14.8 10.9A6.4 6.4 0 1 1 7.1 3.2a5.1 5.1 0 0 0 7.7 7.7Z"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinejoin="round"
        />
      </svg>
    ),
  },
];

export default function SettingsPanel({ mode, onMode }: Props) {
  return (
    <div className="fade-up mx-auto flex w-full max-w-[620px] flex-col gap-4">
      <div className="glass-card px-[18px] pb-[18px] pt-4">
        <div className="section-label mb-1">Appearance</div>
        <div className="mb-3.5 text-[11.5px]" style={{ color: "var(--txt3)" }}>
          System follows the macOS appearance and switches live.
        </div>
        <div
          className="grid grid-cols-3 gap-2"
          role="radiogroup"
          aria-label="Appearance"
        >
          {OPTIONS.map((o) => {
            const active = mode === o.key;
            return (
              <button
                key={o.key}
                role="radio"
                aria-checked={active}
                onClick={() => onMode(o.key)}
                className="btn-focus flex flex-col items-center gap-1.5 rounded-[13px] px-3 py-3.5 transition-all hover:bg-(--track)"
                style={
                  active
                    ? {
                        background: "var(--sel)",
                        boxShadow: "inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--sel-edge)",
                      }
                    : {
                        background: "var(--panel)",
                        boxShadow: "0 0 0 1px var(--panel-edge)",
                      }
                }
              >
                <span style={{ color: active ? "var(--txt)" : "var(--txt2)" }}>{o.icon}</span>
                <span
                  className="text-[12px] font-semibold"
                  style={{ color: active ? "var(--txt)" : "var(--txt2)" }}
                >
                  {o.label}
                </span>
                <span className="mono text-[9.5px]" style={{ color: "var(--txt3)" }}>
                  {o.sub}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      <div className="glass-card flex items-center gap-4 px-[18px] py-4">
        <LogoMark size={34} />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="text-[13px] font-semibold" style={{ color: "var(--txt)" }}>
              Storage Manager
            </span>
            <span className="mono text-[10.5px]" style={{ color: "var(--txt3)" }}>
              {__APP_VERSION__}
            </span>
          </div>
          <div className="text-[11.5px]" style={{ color: "var(--txt2)" }}>
            The macOS storage pane — but honest, drillable, and able to actually fix things.
          </div>
          <div className="mono selectable mt-1 text-[10.5px]" style={{ color: "var(--txt3)" }}>
            github.com/onembyte/storage-manager · MIT
          </div>
        </div>
      </div>
    </div>
  );
}
