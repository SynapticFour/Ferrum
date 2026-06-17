import { apiGet, apiPost } from '@/api/client';

export interface DatasetCatalogEntry {
  id: string;
  name: string;
  description?: string;
  duo_codes: string[];
  external_id?: string;
  dac_group?: string;
  auto_approve_enabled?: boolean;
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
}

export interface AccessStatus {
  ads_available: boolean;
  ads_base_url?: string;
  message?: string;
}

export function getAccessStatus() {
  return apiGet<AccessStatus>('/access/v1/status');
}

export function listCatalogDatasets() {
  return apiGet<{ datasets: DatasetCatalogEntry[] }>('/access/v1/catalog/datasets');
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

export function submitAccessRequest(body: {
  researcher_id: string;
  dataset_id: string;
  project_id: string;
  justification?: string;
}) {
  return apiPost<AccessRequest>('/access/v1/access-requests', body);
}
