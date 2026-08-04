import { useEffect, useRef, useState } from "react";
import type { Card, DryRun } from "../types";
import { fmtKB, shortenPath } from "../format";

interface Props {
  card: Card;
  dryRun: DryRun;
  freeKb: number;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

const grad = (color: string) =>
  `linear-gradient(180deg, color-mix(in srgb, ${color} 62%, transparent), color-mix(in srgb, ${color} 31%, transparent))`;
const ring = (color: string) => `color-mix(in srgb, ${color} 45%, transparent)`;

const VARIANT: Record<DryRun["method"], { color: string; label: string; fg: string }> = {
  trash: { color: "var(--good)", label: "Move to Trash", fg: "var(--on-good)" },
  delete: { color: "var(--danger)", label: "Delete permanently", fg: "#fff" },
  command: { color: "var(--accent)", label: "Run command", fg: "#fff" },
};

const METHOD_TEXT: Record<DryRun["method"], string> = {
  trash: "Everything below moves to the Finder Trash — reversible until you empty it.",
  delete:
    "Deleted permanently. Allowed only because these are known-regenerable folders at their audited paths.",
  command: "Runs this exact command — nothing else:",
};

/** Top-drop confirmation sheet. */
export default function Sheet({ card, dryRun, freeKb, busy, onConfirm, onCancel }: Props) {
  const v = VARIANT[dryRun.method];
  const needsAck = card.tier === "with-care";
  const [acked, setAcked] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);

  // Keyboard users land on the harmless button first.
  useEffect(() => cancelRef.current?.focus(), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onCancel]);

  return (
    <div
      className="absolute inset-0 z-50"
      style={{ background: "rgba(0,0,0,.34)", backdropFilter: "blur(3px)" }}
      onClick={busy ? undefined : onCancel}
      role="dialog"
      aria-modal="true"
      aria-label={card.title}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="sheet-in absolute left-1/2 top-0 flex max-h-[88%] w-[472px] flex-col rounded-b-[20px] p-6 pb-5"
        style={{
          background: "linear-gradient(170deg, var(--glass-a), var(--glass-b)), var(--win)",
          backdropFilter: "blur(50px) saturate(200%)",
          WebkitBackdropFilter: "blur(50px) saturate(200%)",
          boxShadow:
            "0 40px 90px -20px rgba(0,0,0,.7), inset 0 -1px 0 var(--edge-lo), 0 0 0 1px var(--edge)",
        }}
      >
        <div
          className="mb-3.5 flex h-11 w-11 flex-none items-center justify-center rounded-[13px]"
          style={{
            background: `color-mix(in srgb, ${v.color} 16%, transparent)`,
            boxShadow: `0 0 0 1px color-mix(in srgb, ${v.color} 30%, transparent)`,
          }}
        >
          {dryRun.method === "command" ? (
            <svg width="22" height="22" viewBox="0 0 22 22" fill="none" style={{ color: v.color }}>
              <path
                d="M4.5 6.5 9 11l-4.5 4.5M11.5 15.5h6"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          ) : (
            <svg width="22" height="22" viewBox="0 0 22 22" fill="none" style={{ color: v.color }}>
              <path
                d="M4.4 6.4h13.2M8.6 6.4V4.6h4.8v1.8M6 6.4l.9 11.2h8.2L16 6.4"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          )}
        </div>

        <div className="text-[15px] font-semibold" style={{ color: "var(--txt)" }}>
          {card.title}
        </div>
        <div className="mt-1.5 text-[11.5px] leading-relaxed" style={{ color: "var(--txt2)" }}>
          {dryRun.warning ? `${dryRun.warning} ` : ""}
          {METHOD_TEXT[dryRun.method]}
        </div>

        {dryRun.command && (
          <code
            className="mono selectable mt-2.5 block rounded-[10px] px-3 py-2 text-[11.5px]"
            style={{ background: "var(--inset-panel)", color: "var(--txt)", boxShadow: "0 0 0 1px var(--edge-lo)" }}
          >
            $ {dryRun.command}
          </code>
        )}

        <div className="inset-panel mono mt-4 flex flex-none flex-col gap-[7px] p-3.5 text-[11px]">
          <div className="flex justify-between">
            <span style={{ color: "var(--txt3)" }}>locations</span>
            <span style={{ color: "var(--txt)" }}>{dryRun.entries.length || card.paths.length}</span>
          </div>
          <div className="flex justify-between">
            <span style={{ color: "var(--txt3)" }}>total size</span>
            <span style={{ color: "var(--txt)" }}>
              {dryRun.total_kb > 0 ? fmtKB(dryRun.total_kb) : "unknown"}
            </span>
          </div>
          <div className="flex justify-between">
            <span style={{ color: "var(--txt3)" }}>space reclaimed</span>
            <span className="font-semibold" style={{ color: "var(--good)" }}>
              {dryRun.total_kb > 0 ? fmtKB(dryRun.total_kb) : "—"}
            </span>
          </div>
          <div className="flex justify-between">
            <span style={{ color: "var(--txt3)" }}>free after</span>
            <span className="font-semibold" style={{ color: "var(--txt)" }}>
              {fmtKB(freeKb + dryRun.total_kb)}
            </span>
          </div>
        </div>

        {dryRun.entries.length > 0 && (
          <div className="inset-panel selectable mt-2.5 min-h-0 overflow-y-auto p-3" style={{ maxHeight: 170 }}>
            {dryRun.entries.map((e) => (
              <div key={e.path} className="flex items-center gap-3 py-[3px]">
                <span
                  className="mono min-w-0 flex-1 truncate text-[10.5px]"
                  style={{ color: "var(--txt3)" }}
                >
                  {shortenPath(e.path)}
                </span>
                <span
                  className="mono flex-none text-[10.5px] font-medium"
                  style={{ color: "var(--txt2)" }}
                >
                  {fmtKB(e.size_kb)}
                </span>
              </div>
            ))}
          </div>
        )}

        {card.proof && (
          <pre
            className="inset-panel mono selectable mt-2.5 max-h-[110px] overflow-y-auto whitespace-pre-wrap p-3 text-[10.5px] leading-relaxed"
            style={{ color: "var(--txt3)" }}
          >
            {card.proof}
          </pre>
        )}

        <div className="mt-4 flex flex-none items-center gap-2.5">
          {needsAck && (
            <button
              onClick={() => setAcked(!acked)}
              className="btn-focus flex items-center gap-2 text-left"
            >
              <span
                className="flex h-4 w-4 flex-none items-center justify-center rounded-[5px] transition-all"
                style={
                  acked
                    ? {
                        background: "var(--accent)",
                        boxShadow: "0 0 0 1px var(--sel-edge), 0 2px 6px -2px var(--accent)",
                      }
                    : {
                        background: "var(--track)",
                        boxShadow: "inset 0 1px 0 var(--edge-hi), 0 0 0 1px var(--edge-lo)",
                      }
                }
              >
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 10 10"
                  fill="none"
                  style={{ opacity: acked ? 1 : 0 }}
                >
                  <path
                    d="M1.6 5.2 3.9 7.5 8.4 2.6"
                    stroke="#fff"
                    strokeWidth="1.8"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </span>
              <span className="text-[11.5px]" style={{ color: "var(--txt2)" }}>
                I read the list — do it
              </span>
            </button>
          )}
          <div className="grow" />
          <button
            ref={cancelRef}
            onClick={onCancel}
            disabled={busy}
            className="glass-chip btn-focus flex h-[31px] items-center rounded-[9px] px-[17px] text-[12px] font-medium disabled:opacity-50"
            style={{ color: "var(--txt)" }}
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={busy || (needsAck && !acked)}
            className="btn-focus relative flex h-[31px] items-center overflow-hidden rounded-[9px] px-[17px] text-[12px] font-semibold transition-opacity disabled:opacity-40"
            style={{
              background: grad(v.color),
              boxShadow: `inset 0 1px 0 rgba(255,255,255,.5), 0 0 0 1px ${ring(v.color)}, 0 8px 20px -8px ${ring(v.color)}`,
              color: v.fg,
            }}
          >
            {!busy && <div className="sheen" style={{ animationDuration: "5s" }} />}
            <span className="relative">{busy ? "Working…" : v.label}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
