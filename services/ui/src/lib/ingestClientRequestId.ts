/** URL-safe base64 (no padding) for embedding auth `sub` in ingest idempotency keys. */
function encodeSub(sub: string): string {
  const bytes = new TextEncoder().encode(sub);
  let binary = '';
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** Client request id scoped to the signed-in user when `sub` is known. */
export function ingestClientRequestId(
  kind: 'upload' | 'register',
  sub?: string | null,
): string {
  const prefix = kind === 'upload' ? 'ferrum-ui' : 'ferrum-ui-register';
  const owner = sub?.trim() ? encodeSub(sub.trim()) : 'anon';
  return `${prefix}:${owner}:${crypto.randomUUID()}`;
}
