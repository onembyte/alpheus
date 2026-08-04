import type { Tier } from "./types";

/** Single source of truth for how each safety tier looks and reads. */
export const TIER_META: Record<
  Tier,
  { label: string; color: string; heading: string; sub: string }
> = {
  safe: {
    label: "Safe",
    color: "var(--good)",
    heading: "Safe to reclaim",
    sub: "Regenerable caches and build artifacts — everything comes back on demand.",
  },
  "with-care": {
    label: "Needs a decision",
    color: "var(--warn)",
    heading: "Needs a decision",
    sub: "Removable, but read what dies first. The confirm sheet lists it exactly.",
  },
  manual: {
    label: "Manual",
    color: "var(--txt3)",
    heading: "Manual / informational",
    sub: "This app won't touch these — it just explains where the space is.",
  },
};

export const TIER_ORDER: Tier[] = ["safe", "with-care", "manual"];

/**
 * Categorical ramp. Colors are assigned to cards by descending size at
 * scan time and stay with the card for the whole session (color follows the
 * entity, never its rank in a filtered view).
 */
export const CAT_COLORS = [
  "var(--c1)",
  "var(--c2)",
  "var(--c3)",
  "var(--c4)",
  "var(--c5)",
  "var(--c6)",
  "var(--c8)",
  "var(--c9)",
] as const;

/** Muted slot reserved for "everything else on disk" in ring + capacity bar. */
export const OTHER_COLOR = "var(--c7)";

export function cardColor(index: number): string {
  return CAT_COLORS[index % CAT_COLORS.length];
}
