import { useEffect } from 'react';
import { Link } from '@tanstack/react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Loader2 } from 'lucide-react';
import { listIngestJobs, type IngestJob } from '@/api/ingest';
import { useIngestJobsStore } from '@/stores/ingestJobs';
import { ProblemReportPanel } from '@/components/ProblemReportPanel';
import { useI18n } from '@/i18n/I18nProvider';

const TERMINAL = new Set(['succeeded', 'failed']);
const DISMISS_MS = 8000;

function objectIdFromJob(job: IngestJob): string | undefined {
  return job.result?.object_ids?.[0];
}

function syncJobToStore(
  job: IngestJob,
  trackJob: ReturnType<typeof useIngestJobsStore.getState>['trackJob'],
  updateJob: ReturnType<typeof useIngestJobsStore.getState>['updateJob'],
) {
  const kind = job.job_type === 'upload' ? 'upload' : 'register';
  const existing = useIngestJobsStore.getState().jobs.find((j) => j.jobId === job.job_id);
  if (!existing) {
    trackJob({
      jobId: job.job_id,
      status: job.status,
      kind,
      objectId: objectIdFromJob(job),
    });
  } else {
    updateJob(job.job_id, {
      status: job.status,
      objectId: objectIdFromJob(job),
    });
  }
}

export function IngestJobsBanner({
  onSucceeded,
}: {
  onSucceeded?: (objectId: string) => void;
}) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const jobs = useIngestJobsStore((s) => s.jobs);
  const trackJob = useIngestJobsStore((s) => s.trackJob);
  const updateJob = useIngestJobsStore((s) => s.updateJob);
  const removeJob = useIngestJobsStore((s) => s.removeJob);

  const { data: remoteJobs } = useQuery({
    queryKey: ['ingest', 'jobs'],
    queryFn: async () => (await listIngestJobs()).jobs,
    refetchInterval: (query) => {
      const list = query.state.data ?? [];
      const hasActive = list.some((j) => !TERMINAL.has(j.status));
      return hasActive ? 2000 : 15000;
    },
    retry: false,
  });

  useEffect(() => {
    if (!remoteJobs?.length) return;
    for (const job of remoteJobs) {
      if (!TERMINAL.has(job.status)) {
        syncJobToStore(job, trackJob, updateJob);
      }
    }
  }, [remoteJobs, trackJob, updateJob]);

  const activeIds = jobs
    .filter((j) => !TERMINAL.has(j.status))
    .map((j) => j.jobId)
    .join(',');

  useEffect(() => {
    if (!activeIds) return;

    let cancelled = false;

    const poll = async () => {
      while (!cancelled) {
        const active = useIngestJobsStore
          .getState()
          .jobs.filter((j) => !TERMINAL.has(j.status));
        if (!active.length) break;

        for (const job of active) {
          try {
            const res = await fetch(`/api/v1/ingest/jobs/${encodeURIComponent(job.jobId)}`);
            if (!res.ok) continue;
            const data = (await res.json()) as IngestJob;
            const objectId = objectIdFromJob(data);
            updateJob(job.jobId, { status: data.status, objectId });
            if (TERMINAL.has(data.status)) {
              updateJob(job.jobId, { finishedAt: Date.now() });
              if (data.status === 'succeeded') {
                void qc.invalidateQueries({ queryKey: ['drs', 'objects'] });
                void qc.invalidateQueries({ queryKey: ['ingest', 'jobs'] });
                if (objectId) onSucceeded?.(objectId);
              }
              window.setTimeout(() => removeJob(job.jobId), DISMISS_MS);
            }
          } catch {
            /* retry */
          }
        }
        await new Promise((r) => setTimeout(r, 800));
      }
    };

    void poll();
    return () => {
      cancelled = true;
    };
  }, [activeIds, onSucceeded, qc, removeJob, updateJob]);

  if (!jobs.length) return null;

  return (
    <div className="rounded-md border border-primary/30 bg-primary/5 px-3 py-2 text-sm space-y-2">
      <p className="font-medium">{t('data.jobsTitle')}</p>
      <ul className="space-y-1">
        {jobs.map((job) => {
          const running = !TERMINAL.has(job.status);
          const failed = job.status === 'failed';
          return (
            <li key={job.jobId} className="space-y-1">
              <div className="flex flex-wrap items-center gap-2">
              {running && <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />}
              <span className={failed ? 'text-destructive' : 'text-muted-foreground'}>
                {running
                  ? t('data.ingestJobRunning', { jobId: job.jobId, status: job.status })
                  : failed
                    ? t('data.ingestJobFailed')
                    : t('data.registerSuccess', { id: job.objectId ?? job.jobId })}
              </span>
              {job.objectId && (
                <Link
                  to={`/data/objects/${job.objectId}` as any}
                  className="text-primary hover:underline text-xs"
                >
                  {t('data.openObject')}
                </Link>
              )}
              <button
                type="button"
                className="ms-auto text-xs underline opacity-70"
                onClick={() => removeJob(job.jobId)}
              >
                {t('common.dismiss')}
              </button>
              </div>
              {failed && (
                <ProblemReportPanel
                  errorMessage={t('data.ingestJobFailed')}
                  context="data-ingest-job"
                  extra={{ job_id: job.jobId, status: job.status }}
                />
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
