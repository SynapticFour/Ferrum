import { useAuthStore } from '@/stores/auth';

const PREVIEW_MAX_BYTES = 256_000;

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
    m.includes('csv') ||
    m.includes('tab-separated')
  );
}

export function canPreviewFile(name: string, mime?: string | null, size?: number | null): boolean {
  if (size != null && size > PREVIEW_MAX_BYTES) return false;
  return isPreviewableName(name) || isPreviewableMime(mime);
}

export function drsStreamUrl(objectId: string, inline = false): string {
  const base = `/ga4gh/drs/v1/objects/${encodeURIComponent(objectId)}/stream`;
  return inline ? `${base}?inline=true` : base;
}

export async function fetchWithAuth(path: string): Promise<Response> {
  const jwt = useAuthStore.getState().passportJwt;
  return fetch(path, {
    headers: jwt ? { Authorization: `Bearer ${jwt}` } : {},
  });
}

export async function fetchPreviewText(path: string, truncatedLabel: string): Promise<string> {
  const res = await fetchWithAuth(path);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const text = await res.text();
  if (text.length > PREVIEW_MAX_BYTES) {
    return `${text.slice(0, PREVIEW_MAX_BYTES)}\n\n… (${truncatedLabel})`;
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
