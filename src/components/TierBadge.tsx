import type { Tier } from "../types";
import { TIER_META } from "../tiers";

export default function TierBadge({ tier }: { tier: Tier }) {
  const m = TIER_META[tier];
  return (
    <span
      className="glass-chip inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-[10.5px] font-medium"
      style={{ color: "var(--txt2)" }}
    >
      <span
        className="h-1.5 w-1.5 rounded-full"
        style={{ background: m.color, boxShadow: `0 0 6px ${m.color}` }}
      />
      {m.label}
    </span>
  );
}
