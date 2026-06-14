/** Deep-merge plain objects (string leaves or nested records). Arrays are replaced, not merged. */
export function mergeDeep<T extends Record<string, unknown>>(base: T, patch: Partial<T>): T {
  const out: Record<string, unknown> = { ...base };
  for (const key of Object.keys(patch) as Array<keyof T>) {
    const pv = patch[key];
    if (pv === undefined) continue;
    const bv = base[key];
    if (
      pv !== null &&
      typeof pv === 'object' &&
      !Array.isArray(pv) &&
      bv !== null &&
      typeof bv === 'object' &&
      !Array.isArray(bv)
    ) {
      out[key as string] = mergeDeep(bv as Record<string, unknown>, pv as Record<string, unknown>);
    } else {
      out[key as string] = pv as unknown;
    }
  }
  return out as T;
}
