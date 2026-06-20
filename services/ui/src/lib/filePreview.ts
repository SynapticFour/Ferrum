import { useAuthStore } from '@/stores/auth';

/** Max bytes loaded into the browser for inline text preview (first slice only). */
export const PREVIEW_MAX_BYTES = 256_000;

export function isPreviewableName(name: string): boolean {
  const n = name.toLowerCase();
  return (
    n.endsWith('.txt') ||
    n.endsWith('.log') ||
    n.endsWith('.json') ||
    n.endsWith('.html') ||
    n.endsWith('.csv') ||
    n.endsWith('.tsv') ||
    n.endsWith('.vcf') ||
    n.endsWith('.yaml') ||
    n.endsWith('.yml') ||
    n.endsWith('.cwl') ||
    n.endsWith('.wdl') ||
    n.endsWith('.nf') ||
    n.endsWith('.fasta') ||
    n.endsWith('.fa') ||
    n.endsWith('.fq') ||
    n.endsWith('.fastq')
  );
}

export function isPreviewableMime(mime?: string | null): boolean {
  if (!mime) return false;
  const m = mime.toLowerCase();
  return (
    m.startsWith('text/') ||
    m.includes('json') ||
    m.includes('vcf') ||
    m.includes('yaml') ||
    m.includes('yml') ||
    m.includes('csv') ||
    m.includes('tab-separated')
  );
}

/** File name/MIME looks like inline text — independent of object size. */
export function isPreviewableType(name: string, mime?: string | null): boolean {
  return isPreviewableName(name) || isPreviewableMime(mime);
}

/** Small enough to preview the whole object in one request (legacy helper). */
export function canPreviewFile(name: string, mime?: string | null, size?: number | null): boolean {
  if (size != null && size > PREVIEW_MAX_BYTES) return false;
  return isPreviewableType(name, mime);
}

/** True when Ferrum can inline-preview via DRS /stream (managed storage only). */
export function canStreamPreview(
  storageKind: 'managed' | 'url' | 'unknown',
  name: string,
  mime?: string | null,
  _size?: number | null,
): boolean {
  if (storageKind === 'url') return false;
  if (!isPreviewableType(name, mime)) return false;
  return storageKind === 'managed' || storageKind === 'unknown';
}

export function wouldPreviewByType(name: string, mime?: string | null, _size?: number | null): boolean {
  return isPreviewableType(name, mime);
}

export function drsStreamUrl(objectId: string, inline = false): string {
  const base = `/ga4gh/drs/v1/objects/${encodeURIComponent(objectId)}/stream`;
  return inline ? `${base}?inline=true` : base;
}

export async function fetchWithAuth(path: string, init?: RequestInit): Promise<Response> {
  const jwt = useAuthStore.getState().passportJwt;
  const headers = new Headers(init?.headers);
  if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
  return fetch(path, { ...init, headers });
}

function appendTruncated(text: string, truncatedLabel: string): string {
  return `${text.slice(0, PREVIEW_MAX_BYTES)}\n\n… (${truncatedLabel})`;
}

/** Read at most PREVIEW_MAX_BYTES from DRS /stream without buffering the whole object. */
export async function fetchPreviewText(path: string, truncatedLabel: string): Promise<string> {
  const res = await fetchWithAuth(path, {
    headers: { Range: `bytes=0-${PREVIEW_MAX_BYTES - 1}` },
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);

  const contentLength = res.headers.get('content-length');
  const totalBytes = contentLength ? Number.parseInt(contentLength, 10) : undefined;
  const likelyTruncated = totalBytes != null && totalBytes > PREVIEW_MAX_BYTES;

  if (!res.body) {
    const text = await res.text();
    if (likelyTruncated || text.length > PREVIEW_MAX_BYTES) {
      return appendTruncated(text, truncatedLabel);
    }
    return text;
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let text = '';
  try {
    while (text.length < PREVIEW_MAX_BYTES) {
      const { done, value } = await reader.read();
      if (done) break;
      text += decoder.decode(value, { stream: true });
      if (text.length >= PREVIEW_MAX_BYTES) {
        text = text.slice(0, PREVIEW_MAX_BYTES);
        break;
      }
    }
  } finally {
    await reader.cancel().catch(() => {});
  }
  text += decoder.decode();

  if (likelyTruncated || text.length >= PREVIEW_MAX_BYTES) {
    return `${text}\n\n… (${truncatedLabel})`;
  }
  return text;
}

export async function downloadWithAuth(path: string, filename: string): Promise<void> {
  const res = await fetchWithAuth(path);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
