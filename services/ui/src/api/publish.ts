import { apiPost } from '@/api/client';

export interface PublishDatasetRequest {
  object_id: string;
  name: string;
  description?: string;
  duo_codes: string[];
  visibility?: 'draft' | 'institute' | 'public';
  dac_group?: string;
  auto_approve_enabled?: boolean;
}

export interface PublishDatasetResponse {
  ads_dataset_id: string;
  object_id: string;
  visibility: string;
}

export function publishDataset(body: PublishDatasetRequest) {
  return apiPost<PublishDatasetResponse>('/api/v1/datasets/publish', body);
}
