import type { DrsObject } from '@/api/types';

export type DrsStorageKind = 'managed' | 'url' | 'unknown';

function accessUrl(obj: DrsObject): string {
  const am = obj.access_methods?.find((m) => m.type === 'https');
  return am?.access_url?.url ?? '';
}

/** Whether Ferrum stores bytes (MinIO/local) vs external URL reference only. */
export function drsStorageKind(obj: DrsObject): DrsStorageKind {
  const backend = (obj as DrsObject & { storage_backend?: string }).storage_backend;
  if (backend === 'url') return 'url';
  if (backend === 's3' || backend === 'local') return 'managed';

  const url = accessUrl(obj);
  // Managed objects stream via Ferrum gateway relay URLs.
  if (url.includes('/ga4gh/drs/v1/objects/') && url.includes('/access/')) return 'managed';

  const desc = (obj.description ?? '').toLowerCase();
  const name = (obj.name ?? '').toLowerCase();
  if (desc.includes('url pointer') || desc.includes('not a bam') || desc.includes('not a vcf')) {
    return 'url';
  }
  if (name.includes('readme') || name.includes('external alignment')) return 'url';
  if (
    url.startsWith('http') &&
    (url.includes('1000genomes') ||
      url.includes('raw.githubusercontent.com') ||
      url.includes('ftp.'))
  ) {
    return 'url';
  }
  if (url.startsWith('http')) return 'managed';
  return 'unknown';
}

export function isUrlBackedDrsObject(obj: DrsObject | undefined): boolean {
  return !!obj && drsStorageKind(obj) === 'url';
}
