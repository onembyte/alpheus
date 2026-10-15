import type { Card } from "../types";

/** The last finished scan's cards, as the backend kept them (last_state). */
export interface CardSnapshot {
  cards: Card[];
  /** When that scan ran (Unix seconds). */
  at: number;
}

export interface ShownCards {
  cards: Card[] | null;
  /** Showing the kept cards because the new scan hasn't landed yet. */
  fromSnapshot: boolean;
  at: number | null;
}

/**
 * What the reclaim list shows: the live scan once it has landed, even when it
 * found nothing; until then, the last scan's cards so the app opens on them.
 */
export function shownCards(live: Card[] | null, snap: CardSnapshot | null): ShownCards {
  if (live) return { cards: live, fromSnapshot: false, at: null };
  if (snap) return { cards: snap.cards, fromSnapshot: true, at: snap.at };
  return { cards: null, fromSnapshot: false, at: null };
}
