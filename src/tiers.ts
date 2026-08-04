import type { Tier } from "./types";

/** Single source of truth for how each safety tier looks and reads. */
export const TIER_META: Record<
  Tier,
  { label: string; dot: string; text: string; heading: string; sub: string }
> = {
  safe: {
    label: "Safe",
    dot: "bg-emerald-400",
    text: "text-emerald-300",
    heading: "Safe to reclaim",
    sub: "Regenerable caches and build artifacts — everything comes back on demand.",
  },
  "with-care": {
    label: "Needs a decision",
    dot: "bg-amber-400",
    text: "text-amber-300",
    heading: "Needs a decision",
    sub: "Removable, but read what dies first. The confirm dialog lists it exactly.",
  },
  manual: {
    label: "Manual",
    dot: "bg-neutral-400",
    text: "text-neutral-400",
    heading: "Manual / informational",
    sub: "This app won't touch these — it just explains where the space is.",
  },
};

export const TIER_ORDER: Tier[] = ["safe", "with-care", "manual"];
