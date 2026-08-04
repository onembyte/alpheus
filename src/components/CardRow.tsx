import { useState } from "react";
import type { Card } from "../types";
import { fmtKB, shortenPath } from "../format";
import TierBadge from "./TierBadge";

interface Props {
  card: Card;
  color: string;
  maxKb: number;
  onAction: (card: Card) => void;
}

export default function CardRow({ card, color, maxKb, onAction }: Props) {
  const [expanded, setExpanded] = useState(false);
  const actionable = card.action !== "explain";
  const pct = maxKb > 0 ? Math.max(2, (card.size_kb / maxKb) * 100) : 0;
  const pathLine =
    card.paths.length > 0
      ? shortenPath(card.paths[0]) + (card.paths.length > 1 ? ` · +${card.paths.length - 1}` : "")
      : (card.command_display ?? "");

  return (
    <div className="hairline-b last:border-0">
      <div className="flex items-center gap-3.5 py-2.5">
        <div
          className="h-[22px] w-2 flex-none rounded-[3px]"
          style={{ background: color }}
        />
        <div className="w-[190px] flex-none">
          <div className="truncate text-[12.5px] font-medium" style={{ color: "var(--txt)" }}>
            {card.title}
          </div>
          <div className="mono truncate text-[9.5px]" style={{ color: "var(--txt3)" }}>
            {pathLine}
          </div>
        </div>
        <div
          className="h-[9px] min-w-0 flex-1 overflow-hidden rounded-[5px]"
          style={{ background: "var(--track)" }}
        >
          {card.size_kb > 0 && (
            <div
              className="bar-in h-full rounded-[5px]"
              style={{
                width: `${pct}%`,
                background: `linear-gradient(90deg, ${color}, color-mix(in oklch, ${color}, white 22%))`,
              }}
            />
          )}
        </div>
        <div
          className="mono w-[70px] flex-none text-right text-[12px] font-semibold"
          style={{ color: "var(--txt)" }}
        >
          {card.size_kb > 0 ? fmtKB(card.size_kb) : "—"}
        </div>
        <div className="hidden w-[120px] flex-none justify-end xl:flex">
          <TierBadge tier={card.tier} />
        </div>
        <div className="flex w-[86px] flex-none justify-end">
          {actionable ? (
            <button
              onClick={() => onAction(card)}
              className="glass-chip btn-focus h-7 rounded-lg px-3 text-[11.5px] font-semibold"
              style={{ color: "var(--txt)" }}
            >
              {card.tier === "safe" ? "Reclaim" : "Review"}
            </button>
          ) : (
            <span className="mono text-[10px]" style={{ color: "var(--txt3)" }}>
              info only
            </span>
          )}
        </div>
        <button
          onClick={() => setExpanded(!expanded)}
          aria-label={expanded ? "Hide details" : "Show details"}
          className="btn-focus flex h-6 w-6 flex-none items-center justify-center rounded-md transition-colors hover:bg-(--track)"
          style={{ color: "var(--txt3)" }}
        >
          <svg
            width="11"
            height="11"
            viewBox="0 0 12 12"
            fill="none"
            style={{
              transform: expanded ? "rotate(180deg)" : "none",
              transition: "transform .18s ease",
            }}
          >
            <path
              d="M2.4 4.4 6 8l3.6-3.6"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </button>
      </div>

      {expanded && (
        <div className="inset-panel selectable mb-2.5 flex flex-col gap-2 p-3">
          <div className="text-[11.5px] leading-relaxed" style={{ color: "var(--txt2)" }}>
            {card.description}
          </div>
          {card.command_display && (
            <code className="mono text-[10.5px]" style={{ color: "var(--txt)" }}>
              $ {card.command_display}
            </code>
          )}
          {card.paths.length > 0 && (
            <ul className="mono space-y-0.5 text-[10.5px] leading-relaxed" style={{ color: "var(--txt3)" }}>
              {card.paths.map((p) => (
                <li key={p} className="truncate">
                  {shortenPath(p)}
                </li>
              ))}
            </ul>
          )}
          {card.proof && (
            <pre
              className="mono whitespace-pre-wrap text-[10.5px] leading-relaxed"
              style={{ color: "var(--txt3)" }}
            >
              {card.proof}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
