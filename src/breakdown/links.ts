import type { Card } from "../types";

export interface Link {
  /** What the linked cards would free together. */
  kb: number;
  /** Biggest first. */
  cards: Card[];
}

/** The tree's root holds everything, so it never carries a link. */
const ROOT = 0;

/**
 * Which reclaim cards each storage row holds entirely. `chains` maps each
 * card path to the rows holding it, root first (empty when the walk doesn't
 * cover it). A card counts in a row only when every one of its paths sits in
 * that row, so a row never claims space it doesn't hold.
 */
export function cardLinks(cards: Card[], chains: Record<string, number[]>): Map<number, Link> {
  const links = new Map<number, Link>();
  for (const c of cards) {
    if (c.action === "explain" || c.size_kb <= 0 || c.paths.length === 0) continue;
    const sets = c.paths.map((p) => chains[p] ?? []);
    if (sets.some((s) => s.length === 0)) continue;
    const common = sets.reduce((acc, s) => acc.filter((id) => s.includes(id)));
    for (const id of common) {
      if (id === ROOT) continue;
      const link = links.get(id) ?? { kb: 0, cards: [] };
      link.kb += c.size_kb;
      link.cards.push(c);
      links.set(id, link);
    }
  }
  for (const link of links.values()) link.cards.sort((a, b) => b.size_kb - a.size_kb);
  return links;
}

/** Why some space is unattributed, or null when every folder was read. */
export function accessNotice(unreadable: number): string | null {
  if (unreadable <= 0) return null;
  const folders = `${unreadable.toLocaleString()} folder${unreadable === 1 ? "" : "s"}`;
  return `${folders} couldn't be read. macOS keeps them private (Mail, Messages, Safari and other apps' data) unless Alpheus has Full Disk Access; their space is counted in Unattributed.`;
}
