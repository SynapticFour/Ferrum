import { create } from 'zustand';

export type IngestJobKind = 'upload' | 'register';

export interface TrackedIngestJob {
  jobId: string;
  status: string;
  kind: IngestJobKind;
  startedAt: number;
  objectId?: string;
  finishedAt?: number;
}

type IngestJobsState = {
  jobs: TrackedIngestJob[];
  trackJob: (job: Omit<TrackedIngestJob, 'startedAt'> & { startedAt?: number }) => void;
  updateJob: (jobId: string, patch: Partial<TrackedIngestJob>) => void;
  removeJob: (jobId: string) => void;
};

export const useIngestJobsStore = create<IngestJobsState>((set) => ({
  jobs: [],
  trackJob: (job) =>
    set((s) => ({
      jobs: [
        ...s.jobs.filter((j) => j.jobId !== job.jobId),
        { ...job, startedAt: job.startedAt ?? Date.now() },
      ],
    })),
  updateJob: (jobId, patch) =>
    set((s) => ({
      jobs: s.jobs.map((j) => (j.jobId === jobId ? { ...j, ...patch } : j)),
    })),
  removeJob: (jobId) =>
    set((s) => ({
      jobs: s.jobs.filter((j) => j.jobId !== jobId),
    })),
}));
