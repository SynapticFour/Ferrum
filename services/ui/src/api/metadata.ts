import { apiFetch, apiGet, apiPut } from '@/api/client';

export type MetadataListItem = {
  alias: string;
  profile: string;
  version: number;
  content_sha256: string;
  created_time: string;
  updated_time?: string | null;
};

export type MetadataListResponse = {
  items: MetadataListItem[];
  count: number;
  limit: number;
  offset: number;
};

export type MetadataSubmission = MetadataListItem & {
  document: unknown;
};

export type MetadataVersionItem = {
  version: number;
  content_sha256: string;
  created_time: string;
  is_current: boolean;
};

export type MetadataVersionList = {
  alias: string;
  items: MetadataVersionItem[];
};

export type MetadataVersionDocument = {
  alias: string;
  version: number;
  content_sha256: string;
  document: unknown;
};

export type ValidationIssue = {
  path?: string;
  severity?: string;
  message?: string;
};

export function issuesFromUnknown(err: unknown): ValidationIssue[] {
  if (!err || typeof err !== 'object' || !('details' in err)) return [];
  const details = (err as { details?: { issues?: ValidationIssue[] } }).details;
  return Array.isArray(details?.issues) ? details.issues : [];
}

export function listSubmissions(limit = 50): Promise<MetadataListResponse> {
  return apiGet<MetadataListResponse>(
    `/api/v1/metadata/submissions?limit=${encodeURIComponent(String(limit))}`,
  );
}

export function getSubmission(alias: string): Promise<MetadataSubmission> {
  return apiGet<MetadataSubmission>(
    `/api/v1/metadata/submissions/${encodeURIComponent(alias)}`,
  );
}

export function listVersions(alias: string): Promise<MetadataVersionList> {
  return apiGet<MetadataVersionList>(
    `/api/v1/metadata/submissions/${encodeURIComponent(alias)}/versions`,
  );
}

export function getVersion(alias: string, n: number): Promise<MetadataVersionDocument> {
  return apiGet<MetadataVersionDocument>(
    `/api/v1/metadata/submissions/${encodeURIComponent(alias)}/versions/${n}`,
  );
}

export function putSubmission(
  alias: string,
  document: unknown,
  expectedVersion?: number,
): Promise<{ alias: string; version: number; unchanged: boolean }> {
  const headers: Record<string, string> = {};
  if (expectedVersion != null) headers['If-Match'] = `"${expectedVersion}"`;
  return apiFetch(`/api/v1/metadata/submissions/${encodeURIComponent(alias)}`, {
    method: 'PUT',
    body: JSON.stringify(document),
    headers,
  });
}

export function attachMetadataRef(
  objectId: string,
  metadataRef: string | null,
): Promise<{ object_id: string; metadata_ref: string | null }> {
  return apiPut(`/api/v1/metadata/objects/${encodeURIComponent(objectId)}/metadata_ref`, {
    metadata_ref: metadataRef,
  });
}

export function flattenJsonDiff(
  left: unknown,
  right: unknown,
  path = '',
): { path: string; left: string; right: string }[] {
  if (Object.is(left, right)) return [];
  const lObj = left !== null && typeof left === 'object';
  const rObj = right !== null && typeof right === 'object';
  if (!lObj || !rObj || Array.isArray(left) !== Array.isArray(right)) {
    return [
      {
        path: path || '/',
        left: JSON.stringify(left),
        right: JSON.stringify(right),
      },
    ];
  }
  const keys = new Set([...Object.keys(left as object), ...Object.keys(right as object)]);
  const out: { path: string; left: string; right: string }[] = [];
  for (const key of keys) {
    const next = path ? `${path}.${key}` : key;
    out.push(
      ...flattenJsonDiff(
        (left as Record<string, unknown>)[key],
        (right as Record<string, unknown>)[key],
        next,
      ),
    );
  }
  return out;
}
