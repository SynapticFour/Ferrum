import { useEffect, useRef, useState } from 'react';
import { useAuthStore } from '@/stores/auth';

/** Subscribe to WES SSE log stream; falls back gracefully when unavailable. */
export function useLiveRunLogs(runId: string, enabled: boolean) {
  const [lines, setLines] = useState<string[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [unavailable, setUnavailable] = useState(false);

  useEffect(() => {
    if (!enabled || !runId) return;

    const controller = new AbortController();
    let buffer = '';

    (async () => {
      const jwt = useAuthStore.getState().passportJwt;
      try {
        const res = await fetch(`/ga4gh/wes/v1/runs/${encodeURIComponent(runId)}/logs/stream`, {
          headers: jwt ? { Authorization: `Bearer ${jwt}` } : {},
          signal: controller.signal,
        });
        if (!res.ok || !res.body) {
          setUnavailable(true);
          return;
        }
        setStreaming(true);
        setUnavailable(false);
        const reader = res.body.getReader();
        const decoder = new TextDecoder();

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          const parts = buffer.split('\n');
          buffer = parts.pop() ?? '';
          for (const line of parts) {
            if (line.startsWith('data:')) {
              const data = line.slice(5).trim();
              if (data && data !== '[stream closed]') {
                setLines((prev) => [...prev, data]);
              }
            }
          }
        }
      } catch (e) {
        if ((e as Error).name !== 'AbortError') {
          setUnavailable(true);
        }
      } finally {
        setStreaming(false);
      }
    })();

    return () => controller.abort();
  }, [runId, enabled]);

  return { lines, streaming, unavailable };
}

/** Poll ingest job until terminal state. */
export function useIngestJobPoller(
  jobId: string | null,
  onDone: (status: string, result?: { object_ids?: string[] }) => void,
) {
  const doneRef = useRef(false);

  useEffect(() => {
    if (!jobId) return;
    doneRef.current = false;
    let cancelled = false;

    const poll = async () => {
      while (!cancelled && !doneRef.current) {
        try {
          const jwt = useAuthStore.getState().passportJwt;
          const res = await fetch(`/api/v1/ingest/jobs/${encodeURIComponent(jobId)}`, {
            headers: jwt ? { Authorization: `Bearer ${jwt}` } : {},
          });
          if (!res.ok) break;
          const data = (await res.json()) as {
            status: string;
            result?: { object_ids?: string[] };
          };
          if (data.status === 'succeeded' || data.status === 'failed') {
            doneRef.current = true;
            onDone(data.status, data.result);
            break;
          }
        } catch {
          break;
        }
        await new Promise((r) => setTimeout(r, 800));
      }
    };

    void poll();
    return () => {
      cancelled = true;
    };
  }, [jobId, onDone]);
}
