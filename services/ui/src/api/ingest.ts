import { apiGet } from '@/api/client';

export interface IngestJob {
  job_id: string;
  status: string;
  job_type: string;
  result?: { object_ids?: string[] };
  error?: { message?: string };
}

export function listIngestJobs() {
  return apiGet<{ jobs: IngestJob[] }>('/api/v1/ingest/jobs');
}

export function getIngestJob(jobId: string) {
  return apiGet<IngestJob>(`/api/v1/ingest/jobs/${encodeURIComponent(jobId)}`);
}
