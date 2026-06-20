import type { DrsObject } from '@/api/types';

export type DrsStorageKind = 'managed' | 'url' | 'unknown';

function accessUrl(obj: DrsObject): string {
  const am = obj.access_methods?.find((m) => m.type === 'https');
  return am?.access_url?.url ?? '';
}

/** Whether Ferrum stores bytes (MinIO/local) vs external URL reference only. */
export function drsStorageKind(obj: DrsObject): DrsStorageKind {
  const backend = obj.storage_backend?.toLowerCase();
  if (backend === 'url') return 'url';
  if (backend === 's3' || backend === 'local' || backend === 'minio') return 'managed';

  const url = accessUrl(obj);
  // Managed objects are served via Ferrum DRS stream/access endpoints.
  if (url.includes('/ga4gh/drs/v1/objects/')) return 'managed';

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
  // Ingested objects without URL metadata are Ferrum-managed.
  if (obj.storage_backend) return 'managed';
  return 'unknown';
}

export function isUrlBackedDrsObject(obj: DrsObject | undefined): boolean {
  return !!obj && drsStorageKind(obj) === 'url';
}
