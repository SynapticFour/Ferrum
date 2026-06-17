import { apiGet, apiPost } from '@/api/client';

export interface PublishDatasetRequest {
  object_id: string;
  name: string;
  description?: string;
  duo_codes: string[];
  visibility?: 'draft' | 'institute' | 'public';
  dac_group?: string;
  auto_approve_enabled?: boolean;
  index_beacon?: boolean;
  index_variants?: boolean;
}

export interface PublishDatasetResponse {
  ads_dataset_id: string;
  object_id: string;
  visibility: string;
  beacon_indexed?: boolean;
  variants_indexed?: number;
  vcf_index_status?: string;
}

export interface PublishIndexStatusResponse {
  object_id: string;
  vcf_index_status?: string;
  variants_indexed?: number;
}

export function publishDataset(body: PublishDatasetRequest) {
  return apiPost<PublishDatasetResponse>('/api/v1/datasets/publish', body);
}

export function getPublishIndexStatus(objectId: string) {
  return apiGet<PublishIndexStatusResponse>(
    `/api/v1/datasets/publish/${encodeURIComponent(objectId)}/index-status`,
  );
}
