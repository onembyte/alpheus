import type { Tier } from "../types";

const STYLES: Record<Tier, { dot: string; text: string; label: string }> = {
  safe: { dot: "bg-emerald-400", text: "text-emerald-300", label: "Safe" },
  "with-care": { dot: "bg-amber-400", text: "text-amber-300", label: "Needs a decision" },
  manual: { dot: "bg-neutral-400", text: "text-neutral-400", label: "Manual" },
};

export default function TierBadge({ tier }: { tier: Tier }) {
  const s = STYLES[tier];
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border border-neutral-700/70 bg-neutral-800/60 px-2 py-0.5 text-[11px] font-medium ${s.text}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${s.dot}`} />
      {s.label}
    </span>
  );
}
