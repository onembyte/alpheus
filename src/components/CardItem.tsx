import { useState } from "react";
import type { Card } from "../types";
import { fmtKB } from "../format";
import TierBadge from "./TierBadge";

interface Props {
  card: Card;
  onAction: (card: Card) => void;
}

const BUTTON: Record<string, { label: string; cls: string }> = {
  safe: {
    label: "Reclaim…",
    cls: "border-emerald-700/50 bg-emerald-600/15 text-emerald-300 hover:bg-emerald-600/25",
  },
  "with-care": {
    label: "Review…",
    cls: "border-amber-700/50 bg-amber-600/15 text-amber-300 hover:bg-amber-600/25",
  },
};

export default function CardItem({ card, onAction }: Props) {
  const [expanded, setExpanded] = useState(false);
  const btn = card.action === "explain" ? null : BUTTON[card.tier];

  return (
    <div className="flex items-start gap-4 rounded-xl border border-neutral-800 bg-neutral-900 p-4">
      <div className="min-w-0 grow">
        <div className="flex items-center gap-2">
          <h3 className="truncate text-sm font-semibold">{card.title}</h3>
          <TierBadge tier={card.tier} />
        </div>
        <p className="mt-1 text-[13px] leading-relaxed text-neutral-400">{card.description}</p>

        {card.command_display && (
          <code className="mt-2 inline-block rounded-md bg-neutral-800 px-2 py-1 text-[11px] text-neutral-300">
            {card.command_display}
          </code>
        )}

        {(card.paths.length > 0 || card.proof) && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="mt-2 block text-[11px] text-neutral-500 underline decoration-neutral-700 underline-offset-2 hover:text-neutral-300"
          >
            {expanded
              ? "hide details"
              : `${card.paths.length} location${card.paths.length === 1 ? "" : "s"}${card.proof ? " · evidence" : ""}`}
          </button>
        )}

        {expanded && (
          <div className="selectable mt-2 space-y-2 rounded-lg border border-neutral-800 bg-neutral-950/60 p-3">
            {card.paths.length > 0 && (
              <ul className="space-y-0.5 font-mono text-[11px] leading-relaxed text-neutral-400">
                {card.paths.map((p) => (
                  <li key={p} className="truncate">
                    {p}
                  </li>
                ))}
              </ul>
            )}
            {card.proof && (
              <pre className="whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-neutral-500">
                {card.proof}
              </pre>
            )}
          </div>
        )}
      </div>

      <div className="w-32 shrink-0 text-right">
        <div className="text-lg font-semibold tabular-nums">
          {card.size_kb > 0 ? fmtKB(card.size_kb) : "—"}
        </div>
        {btn ? (
          <button
            onClick={() => onAction(card)}
            className={`mt-2 rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors ${btn.cls}`}
          >
            {btn.label}
          </button>
        ) : (
          <div className="mt-2 text-[11px] text-neutral-600">info only</div>
        )}
      </div>
    </div>
  );
}
