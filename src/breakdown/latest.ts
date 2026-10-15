/**
 * Coalesces a stream so only its newest value is applied, at most once per
 * scheduled flush. Walk payloads can arrive faster than a busy window can
 * draw them; applying each one in turn builds a backlog the window then
 * spends minutes working through. Each payload carries the whole current
 * state, so skipping the stale ones in between loses nothing.
 */
export function latestOnly<T>(
  apply: (value: T) => void,
  schedule: (flush: () => void) => void,
): (value: T) => void {
  let newest: { value: T } | null = null;
  return (value: T) => {
    const waiting = newest !== null;
    newest = { value };
    if (waiting) return;
    schedule(() => {
      const take = newest;
      newest = null;
      if (take) apply(take.value);
    });
  };
}
