/** Subsequence match: score `query` against `name` (lower is better; -1 = no match). */
export function fuzzyScore(query: string, name: string): number {
  if (query.length === 0) {
    return 0;
  }
  const q = query.toLowerCase();
  const n = name.toLowerCase();
  let score = 0;
  let last = -1;
  let streak = 0;
  for (const ch of q) {
    const at = n.indexOf(ch, last + 1);
    if (at === -1) {
      return -1;
    }
    streak = at === last + 1 ? streak + 1 : 0;
    score += at - last - 1 - streak;
    last = at;
  }
  return Math.max(score, 0);
}

/** Names matching `query`, best (smallest score) first, stable for ties. */
export function fuzzyFilter(query: string, names: string[]): string[] {
  if (query.length === 0) {
    return names;
  }
  return names
    .map((name, i) => ({ name, i, score: fuzzyScore(query, name) }))
    .filter((x) => x.score >= 0)
    .toSorted((a, b) => a.score - b.score || a.i - b.i)
    .map((x) => x.name);
}
