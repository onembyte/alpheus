import type { Card, Tier } from "./types";

/**
 * The top-bar query language, kept deliberately tiny:
 *   free text        substring match on title, description, paths, id
 *   size:>1GB        size above/below a value (kb/mb/gb/tb)
 *   tier:safe        safe | care | with-care | manual
 * Tokens combine with AND.
 */
export interface Query {
  texts: string[];
  size?: { op: ">" | "<"; kb: number };
  tier?: Tier;
}

const UNIT_KB: Record<string, number> = {
  kb: 1,
  mb: 1024,
  gb: 1024 * 1024,
  tb: 1024 * 1024 * 1024,
};

export function parseQuery(raw: string): Query {
  const q: Query = { texts: [] };
  for (const token of raw.trim().toLowerCase().split(/\s+/)) {
    if (!token) continue;
    const size = /^size:([<>])(\d+(?:\.\d+)?)(kb|mb|gb|tb)?$/.exec(token);
    if (size) {
      q.size = {
        op: size[1] as ">" | "<",
        kb: parseFloat(size[2]) * UNIT_KB[size[3] ?? "gb"],
      };
      continue;
    }
    const tier = /^tier:(safe|care|with-care|manual)$/.exec(token);
    if (tier) {
      q.tier = (tier[1] === "care" ? "with-care" : tier[1]) as Tier;
      continue;
    }
    q.texts.push(token);
  }
  return q;
}

export function cardMatches(card: Card, q: Query): boolean {
  if (q.tier && card.tier !== q.tier) return false;
  if (q.size) {
    if (q.size.op === ">" && !(card.size_kb > q.size.kb)) return false;
    if (q.size.op === "<" && !(card.size_kb < q.size.kb)) return false;
  }
  if (q.texts.length > 0) {
    const haystack = [card.id, card.title, card.description, ...card.paths]
      .join("\n")
      .toLowerCase();
    if (!q.texts.every((t) => haystack.includes(t))) return false;
  }
  return true;
}

export const isEmptyQuery = (q: Query): boolean =>
  q.texts.length === 0 && !q.size && !q.tier;
