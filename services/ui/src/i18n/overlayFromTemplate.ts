/** Build a locale overlay with the same key shape as template, preferring locale then base (en) values. */
export function overlayFromTemplate(
  template: Record<string, unknown>,
  base: Record<string, unknown>,
  locale: Record<string, unknown>,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(template)) {
    if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
      const nested = overlayFromTemplate(
        v as Record<string, unknown>,
        (base[k] as Record<string, unknown>) ?? {},
        (locale[k] as Record<string, unknown>) ?? {},
      );
      if (Object.keys(nested).length > 0) out[k] = nested;
    } else if (Object.prototype.hasOwnProperty.call(locale, k)) {
      out[k] = locale[k];
    } else if (Object.prototype.hasOwnProperty.call(base, k)) {
      out[k] = base[k];
    }
  }
  return out;
}
