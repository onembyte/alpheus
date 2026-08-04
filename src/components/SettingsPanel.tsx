import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import type { ThemeMode } from "../hooks/useTheme";
import type { AppSettings } from "../types";
import LogoMark from "./LogoMark";

/** On/Off pill pair used by the boolean rows. */
function TogglePair({
  value,
  disabled,
  onChange,
}: {
  value: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex gap-2">
      {[
        { label: "Off", v: false },
        { label: "On", v: true },
      ].map((o) => {
        const active = value === o.v;
        return (
          <button
            key={o.label}
            disabled={disabled}
            onClick={() => onChange(o.v)}
            className="btn-focus rounded-[11px] px-5 py-2 text-[12px] font-semibold transition-all hover:bg-(--track) disabled:opacity-50"
            style={
              active
                ? {
                    color: "var(--txt)",
                    background: "var(--sel)",
                    boxShadow: "inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--sel-edge)",
                  }
                : {
                    color: "var(--txt2)",
                    background: "var(--panel)",
                    boxShadow: "0 0 0 1px var(--panel-edge)",
                  }
            }
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

interface Props {
  mode: ThemeMode;
  onMode: (m: ThemeMode) => void;
  settings: AppSettings | null;
  update: (patch: Partial<AppSettings>) => void;
}

const SCAN_INTERVALS: { label: string; secs: number }[] = [
  { label: "Off", secs: 0 },
  { label: "Hourly", secs: 3600 },
  { label: "6 h", secs: 21600 },
  { label: "Daily", secs: 86400 },
  { label: "Weekly", secs: 604800 },
  { label: "Monthly", secs: 2592000 },
];

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

export default function SettingsPanel({ mode, onMode, settings, update }: Props) {
  const [loginItem, setLoginItem] = useState<boolean | null>(null);

  useEffect(() => {
    isEnabled()
      .then(setLoginItem)
      .catch(() => setLoginItem(false));
  }, []);

  const toggleLoginItem = async (v: boolean) => {
    try {
      if (v) await enable();
      else await disable();
      setLoginItem(v);
    } catch {
      setLoginItem(await isEnabled().catch(() => false));
    }
  };

  return (
    <div className="fade-up mx-auto flex w-full max-w-[620px] flex-col gap-4">
      <div className="glass-card px-[18px] pb-[18px] pt-4">
        <div className="section-label mb-3">General</div>
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="text-[12.5px] font-medium" style={{ color: "var(--txt)" }}>
              Launch at login
            </div>
            <div className="text-[11px]" style={{ color: "var(--txt3)" }}>
              Start Alpheus when you log in, so scheduled scans always run.
            </div>
          </div>
          <TogglePair
            value={loginItem ?? false}
            disabled={loginItem === null}
            onChange={toggleLoginItem}
          />
        </div>
        <div className="mt-3.5 flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="text-[12.5px] font-medium" style={{ color: "var(--txt)" }}>
              Menu bar only
            </div>
            <div className="text-[11px]" style={{ color: "var(--txt3)" }}>
              Hide the Dock icon — the window stays reachable from the menu-bar number.
            </div>
          </div>
          <TogglePair
            value={settings?.menu_bar_only ?? false}
            disabled={!settings}
            onChange={(v) => update({ menu_bar_only: v })}
          />
        </div>
      </div>

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

      <div className="glass-card px-[18px] pb-[18px] pt-4">
        <div className="section-label mb-1">Automatic scanning</div>
        <div className="mb-3.5 text-[11.5px]" style={{ color: "var(--txt3)" }}>
          Rescans in the background and keeps the menu-bar number fresh. You'll get a
          notification when free space drops below the threshold.
        </div>
        <div className="flex gap-2" role="radiogroup" aria-label="Automatic scan interval">
          {SCAN_INTERVALS.map((o) => {
            const active = settings?.auto_scan_secs === o.secs;
            return (
              <button
                key={o.secs}
                role="radio"
                aria-checked={active}
                disabled={!settings}
                onClick={() => update({ auto_scan_secs: o.secs })}
                className="btn-focus flex-1 rounded-[11px] px-3 py-2 text-[12px] font-semibold transition-all hover:bg-(--track) disabled:opacity-50"
                style={
                  active
                    ? {
                        color: "var(--txt)",
                        background: "var(--sel)",
                        boxShadow: "inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--sel-edge)",
                      }
                    : {
                        color: "var(--txt2)",
                        background: "var(--panel)",
                        boxShadow: "0 0 0 1px var(--panel-edge)",
                      }
                }
              >
                {o.label}
              </button>
            );
          })}
        </div>
        <div className="mt-3.5 flex items-center gap-2.5">
          <span className="text-[12px]" style={{ color: "var(--txt2)" }}>
            Warn when free space drops below
          </span>
          <input
            type="number"
            min={5}
            max={200}
            step={1}
            disabled={!settings}
            value={settings ? Math.round(settings.notify_below_gb) : 15}
            onChange={(e) => {
              const v = Number(e.target.value);
              if (Number.isFinite(v) && v >= 1) update({ notify_below_gb: v });
            }}
            className="mono inset-panel btn-focus w-16 rounded-[9px] px-2 py-1.5 text-right text-[12px]"
            style={{ color: "var(--txt)", border: "none" }}
            aria-label="Low free space threshold in gigabytes"
          />
          <span className="mono text-[11px]" style={{ color: "var(--txt3)" }}>
            GB
          </span>
        </div>
      </div>

      <div className="glass-card px-[18px] pb-[18px] pt-4">
        <div className="section-label mb-1">Automatic cleanup</div>
        <div className="mb-3.5 text-[11.5px]" style={{ color: "var(--txt3)" }}>
          When an automatic scan finds safe-tier categories you've marked{" "}
          <span className="font-semibold" style={{ color: "var(--txt2)" }}>
            Auto
          </span>{" "}
          on their row, they're cleaned right away — same Trash-first rules as a manual
          click, and every run lands in History. Only the green safe tier can auto-clean;
          command-based cards (like Homebrew) stay manual.
        </div>
        <div className="flex items-center gap-2">
          {[
            { label: "Off", value: false },
            { label: "On", value: true },
          ].map((o) => {
            const active = (settings?.auto_clean ?? false) === o.value;
            return (
              <button
                key={o.label}
                disabled={!settings}
                onClick={() => update({ auto_clean: o.value })}
                className="btn-focus rounded-[11px] px-5 py-2 text-[12px] font-semibold transition-all hover:bg-(--track) disabled:opacity-50"
                style={
                  active
                    ? {
                        color: "var(--txt)",
                        background: "var(--sel)",
                        boxShadow: "inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--sel-edge)",
                      }
                    : {
                        color: "var(--txt2)",
                        background: "var(--panel)",
                        boxShadow: "0 0 0 1px var(--panel-edge)",
                      }
                }
              >
                {o.label}
              </button>
            );
          })}
          <span className="mono ml-1 text-[10.5px]" style={{ color: "var(--txt3)" }}>
            {settings?.auto_clean_ids.length ?? 0} categor
            {(settings?.auto_clean_ids.length ?? 0) === 1 ? "y" : "ies"} marked
          </span>
        </div>
        {settings?.auto_clean && settings.auto_scan_secs === 0 && (
          <div className="mono mt-2.5 text-[10.5px]" style={{ color: "var(--warn)" }}>
            automatic scanning is off — nothing will run until you pick an interval above
          </div>
        )}
      </div>

      <div className="glass-card flex items-center gap-4 px-[18px] py-4">
        <LogoMark size={34} />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="text-[13px] font-semibold" style={{ color: "var(--txt)" }}>
              Alpheus
            </span>
            <span className="mono text-[10.5px]" style={{ color: "var(--txt3)" }}>
              {__APP_VERSION__}
            </span>
          </div>
          <div className="text-[11.5px]" style={{ color: "var(--txt2)" }}>
            The macOS storage pane — but honest, drillable, and able to actually fix things.
          </div>
          <div className="mono selectable mt-1 text-[10.5px]" style={{ color: "var(--txt3)" }}>
            github.com/onembyte/alpheus · MIT
          </div>
        </div>
      </div>
    </div>
  );
}
