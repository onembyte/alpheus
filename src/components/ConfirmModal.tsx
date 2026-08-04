import { useEffect, useState } from "react";
import type { Card, DryRun } from "../types";
import { fmtKB } from "../format";
import TierBadge from "./TierBadge";

interface Props {
  card: Card;
  dryRun: DryRun;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

const METHOD_TEXT: Record<DryRun["method"], string> = {
  trash: "Everything below is moved to the Finder Trash — reversible until you empty it.",
  delete:
    "Everything below is deleted permanently. Allowed only because these are known-regenerable folders at their audited paths.",
  command: "Runs this exact command — nothing else:",
};

const CONFIRM_BTN: Record<DryRun["method"], { label: string; cls: string }> = {
  trash: { label: "Move to Trash", cls: "bg-emerald-600 hover:bg-emerald-500 text-white" },
  delete: { label: "Delete permanently", cls: "bg-red-600 hover:bg-red-500 text-white" },
  command: { label: "Run command", cls: "bg-sky-600 hover:bg-sky-500 text-white" },
};

export default function ConfirmModal({ card, dryRun, busy, onConfirm, onCancel }: Props) {
  const needsAck = card.tier === "with-care";
  const [acked, setAcked] = useState(false);
  const btn = CONFIRM_BTN[dryRun.method];

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      onClick={busy ? undefined : onCancel}
      role="dialog"
      aria-modal="true"
      aria-label={card.title}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="modal-enter flex max-h-[85vh] w-[580px] flex-col rounded-2xl border border-neutral-700 bg-neutral-900 shadow-2xl"
      >
        <div className="flex items-center gap-2 border-b border-neutral-800 px-5 py-4">
          <h2 className="text-sm font-semibold">{card.title}</h2>
          <TierBadge tier={card.tier} />
          <div className="grow" />
          <div className="text-lg font-semibold tabular-nums">
            {dryRun.total_kb > 0 ? fmtKB(dryRun.total_kb) : "size unknown"}
          </div>
        </div>

        <div className="min-h-0 grow overflow-y-auto px-5 py-4">
          {dryRun.warning && (
            <div className="mb-3 rounded-lg border border-amber-700/50 bg-amber-600/10 px-3 py-2 text-[12px] leading-relaxed text-amber-200">
              {dryRun.warning}
            </div>
          )}

          <p className="text-[12px] leading-relaxed text-neutral-400">
            {METHOD_TEXT[dryRun.method]}
          </p>

          {dryRun.command && (
            <code className="selectable mt-2 block rounded-md bg-neutral-950 px-3 py-2 font-mono text-[12px] text-neutral-200">
              {dryRun.command}
            </code>
          )}

          {dryRun.entries.length > 0 && (
            <ul className="selectable mt-3 divide-y divide-neutral-800/70 rounded-lg border border-neutral-800 bg-neutral-950/60">
              {dryRun.entries.map((e) => (
                <li key={e.path} className="flex items-center gap-3 px-3 py-1.5">
                  <span className="min-w-0 grow truncate font-mono text-[11px] text-neutral-400">
                    {e.path}
                  </span>
                  <span className="shrink-0 text-[11px] font-medium tabular-nums text-neutral-300">
                    {fmtKB(e.size_kb)}
                  </span>
                </li>
              ))}
            </ul>
          )}

          {card.proof && (
            <pre className="selectable mt-3 whitespace-pre-wrap rounded-lg border border-neutral-800 bg-neutral-950/60 p-3 font-mono text-[11px] leading-relaxed text-neutral-500">
              {card.proof}
            </pre>
          )}
        </div>

        <div className="flex items-center gap-3 border-t border-neutral-800 px-5 py-4">
          {needsAck && (
            <label className="flex items-center gap-2 text-[12px] text-neutral-300">
              <input
                type="checkbox"
                checked={acked}
                onChange={(e) => setAcked(e.target.checked)}
                className="h-4 w-4 accent-amber-500"
              />
              I read the list — do it
            </label>
          )}
          <div className="grow" />
          <button
            onClick={onCancel}
            disabled={busy}
            className="btn-focus rounded-lg border border-neutral-700 bg-neutral-800 px-4 py-2 text-xs font-medium text-neutral-300 hover:bg-neutral-700 disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={busy || (needsAck && !acked)}
            className={`btn-focus rounded-lg px-4 py-2 text-xs font-semibold transition-colors disabled:opacity-40 ${btn.cls}`}
          >
            {busy ? "Working…" : btn.label}
          </button>
        </div>
      </div>
    </div>
  );
}
