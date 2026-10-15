import { fmtDate } from "../format";
import type { Screen } from "./state";

/**
 * Fixed colors for the Settings categories. A color belongs to its category
 * for good, so a row never changes color as sizes shift during a walk.
 */
const COLORS: Record<string, string> = {
  Applications: "var(--c1)",
  Developer: "var(--c2)",
  Documents: "var(--c3)",
  "iCloud Drive": "var(--c6)",
  Mail: "var(--c4)",
  Messages: "var(--c8)",
  Music: "var(--c5)",
  Photos: "color-mix(in srgb, var(--c5) 55%, var(--c4))",
  macOS: "var(--c9)",
  "System Data": "var(--c7)",
};

const MUTED = "color-mix(in srgb, var(--c7) 55%, transparent)";

export function categoryColor(name: string): string {
  return COLORS[name] ?? MUTED;
}

/** One line on where the numbers come from and how fresh they are. */
export function statusLine(s: Screen): string {
  if (s.running) return `Measuring · ${s.files.toLocaleString()} files`;
  if (s.measuringHere) return `Looking inside ${s.title}`;
  if (s.preview && s.asOf) return `As of ${fmtDate(s.asOf)} · updating`;
  if (s.asOf) return `Measured ${fmtDate(s.asOf)}`;
  return "Not measured yet";
}
