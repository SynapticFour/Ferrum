import { apiFetch, apiGet, apiPost } from '@/api/client';

export interface DatasetCatalogEntry {
  id: string;
  name: string;
  description?: string;
  duo_codes: string[];
  external_id?: string;
  dac_group?: string;
  auto_approve_enabled?: boolean;
  visibility?: 'draft' | 'institute' | 'public';
  resource_type?: 'dataset' | 'compute_pool';
  remote_drs_base_url?: string;
  remote_wes_base_url?: string;
  ads_base_url?: string;
  federation_origin?: string;
}

export interface ResearchProject {
  id: string;
  researcher_id: string;
  name: string;
  description?: string;
  duo_codes: string[];
  created_at: string;
  updated_at: string;
}

export type AccessRequestStatus = 'pending' | 'approved' | 'rejected' | 'escalated';

export interface AccessRequest {
  id: string;
  researcher_id: string;
  dataset_id: string;
  project_id: string;
  status: AccessRequestStatus;
  justification?: string;
  dac_group?: string;
  created_at: string;
  updated_at: string;
}

export interface Grant {
  id: string;
  researcher_id: string;
  dataset_id: string;
  duo_codes: string[];
  created_at: string;
  source?: string;
  resource_scope?: string;
  expires_at?: string;
  dataset_name?: string;
  description?: string;
  external_id?: string;
  resource_type?: 'dataset' | 'compute_pool';
  remote_drs_base_url?: string;
  remote_wes_base_url?: string;
  federation_origin?: string;
  ads_base_url?: string;
}

export interface AccessStatus {
  ads_available: boolean;
  ads_base_url?: string;
  message?: string;
}

export function getAccessStatus() {
  return apiGet<AccessStatus>('/access/v1/status');
}

export function listCatalogDatasets(resourceType?: 'dataset' | 'compute_pool') {
  const qs = resourceType ? `?resource_type=${resourceType}` : '';
  return apiGet<{ datasets: DatasetCatalogEntry[] }>(`/access/v1/catalog/datasets${qs}`);
}

export function listFederatedCatalog() {
  return apiGet<{
    datasets: (DatasetCatalogEntry & {
      federation_origin?: string;
      ads_base_url?: string;
    })[];
    sources: { origin: string; ads_base_url: string }[];
    errors: unknown[];
    duplicates_dropped?: number;
  }>('/access/v1/catalog/federated');
}

export function listMyProjects() {
  return apiGet<{ projects: ResearchProject[] }>('/access/v1/me/projects');
}

export function listMyAccessRequests() {
  return apiGet<{ requests: AccessRequest[] }>('/access/v1/me/access-requests');
}

export function listMyGrants() {
  return apiGet<{ grants: Grant[] }>('/access/v1/me/grants');
}

export function createProject(body: {
  researcher_id: string;
  name: string;
  description?: string;
  duo_codes: string[];
}) {
  return apiPost<ResearchProject>('/access/v1/projects', body);
}

export function submitAccessRequest(
  body: {
    researcher_id: string;
    dataset_id: string;
    project_id: string;
    justification?: string;
  },
  adsBaseUrl?: string,
) {
  return apiFetch<AccessRequest>('/access/v1/access-requests', {
    method: 'POST',
    body: JSON.stringify(body),
    headers: adsBaseUrl ? { 'X-ADS-Base-URL': adsBaseUrl } : undefined,
  });
}

export function federatedDrsUrl(
  remoteDrsBase?: string,
  externalId?: string,
  federationOrigin?: string,
  datasetId?: string,
) {
  const objectId = externalId?.startsWith('drs:') ? externalId.slice(4) : externalId;
  if (!objectId) return null;
  const params = new URLSearchParams();
  if (remoteDrsBase) {
    params.set('base_url', remoteDrsBase.trim().replace(/\/$/, ''));
  } else if (federationOrigin) {
    params.set('origin', federationOrigin);
  } else {
    return null;
  }
  if (datasetId) params.set('dataset_id', datasetId);
  return `/access/v1/federated/drs/objects/${encodeURIComponent(objectId)}?${params}`;
}

export function federatedWesRunsUrl(
  remoteWesBase?: string,
  federationOrigin?: string,
  datasetId?: string,
  adsBaseUrl?: string,
) {
  const params = new URLSearchParams();
  if (remoteWesBase) {
    params.set('base_url', remoteWesBase.trim().replace(/\/$/, ''));
  } else if (federationOrigin) {
    params.set('origin', federationOrigin);
  } else {
    return null;
  }
  if (datasetId) params.set('dataset_id', datasetId);
  if (adsBaseUrl) params.set('ads_base_url', adsBaseUrl);
  return `/access/v1/federated/wes/runs?${params}`;
}
