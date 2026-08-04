import type { Tier } from "../types";
import { TIER_META } from "../tiers";

export default function TierBadge({ tier }: { tier: Tier }) {
  const m = TIER_META[tier];
  return (
    <span
      className={`inline-flex shrink-0 items-center gap-1.5 rounded-full border border-neutral-700/70 bg-neutral-800/60 px-2 py-0.5 text-[11px] font-medium ${m.text}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${m.dot}`} />
      {m.label}
    </span>
  );
}
