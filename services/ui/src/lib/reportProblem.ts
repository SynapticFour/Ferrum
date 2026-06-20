import type { SanitizedConfig } from '@/hooks/useAdminConfig';

export const PROBLEM_REPORT_EMAIL =
  import.meta.env.VITE_PROBLEM_REPORT_EMAIL || 'contact@synapticfour.com';

export const PROBLEM_REPORT_GITHUB_REPO =
  import.meta.env.VITE_PROBLEM_REPORT_GITHUB_REPO || 'SynapticFour/Ferrum';

/** Pilot feedback on by default; set `VITE_ENABLE_PROBLEM_REPORT=false` to hide. */
export function isProblemReportEnabled(): boolean {
  return import.meta.env.VITE_ENABLE_PROBLEM_REPORT !== 'false';
}

export interface ProblemReportApiHint {
  method?: string;
  path?: string;
  status?: number;
}

export interface ProblemReportInput {
  errorMessage: string;
  /** Short label, e.g. `data-import`, `wes-run`, `beacon-query`. */
  context?: string;
  lastApi?: ProblemReportApiHint;
  extra?: Record<string, string | number | boolean | null | undefined>;
  adminConfig?: SanitizedConfig | null;
}

function line(key: string, value: string | number | boolean | null | undefined): string | null {
  if (value === undefined || value === null || value === '') return null;
  return `${key}: ${value}`;
}

export function buildProblemReportBody(input: ProblemReportInput): string {
  const cfg = input.adminConfig;
  const lines = [
    '## Ferrum pilot problem report',
    '',
    line('Error', input.errorMessage),
    line('Context', input.context),
    line('Page', typeof window !== 'undefined' ? window.location.pathname : undefined),
    line('URL', typeof window !== 'undefined' ? window.location.href : undefined),
    line('Time (UTC)', new Date().toISOString()),
    line('Locale', typeof navigator !== 'undefined' ? navigator.language : undefined),
    line('User agent', typeof navigator !== 'undefined' ? navigator.userAgent : undefined),
    '',
    '### Deployment',
    line('Origin', typeof window !== 'undefined' ? window.location.origin : undefined),
    line('Deployment mode', cfg?.deployment_mode),
    line('TES backend', cfg?.compute?.tes_backend),
    line('Crypt4GH ingest ready', cfg?.services?.crypt4gh_ingest_ready),
    line('Storage backend', cfg?.storage?.backend),
    line('Max upload (bytes)', cfg?.ingest?.max_upload_bytes),
    line('Max chunk (bytes)', cfg?.ingest?.max_chunk_bytes),
    '',
    input.lastApi?.path || input.lastApi?.method
      ? [
          '### Last API call',
          line('Method', input.lastApi?.method),
          line('Path', input.lastApi?.path),
          line('Status', input.lastApi?.status),
          '',
        ].filter(Boolean).join('\n')
      : null,
    input.extra && Object.keys(input.extra).length
      ? [
          '### Extra',
          ...Object.entries(input.extra)
            .map(([k, v]) => line(k, v))
            .filter((x): x is string => !!x),
          '',
        ].join('\n')
      : null,
    '---',
    'No JWT, passwords, or file contents are included. Add steps to reproduce if you can.',
  ]
    .flat()
    .filter((x): x is string => x !== null);

  return lines.join('\n');
}

export function buildProblemReportTitle(input: ProblemReportInput): string {
  const ctx = input.context ? `[${input.context}] ` : '';
  const short =
    input.errorMessage.length > 72
      ? `${input.errorMessage.slice(0, 69)}…`
      : input.errorMessage;
  return `Ferrum pilot: ${ctx}${short}`;
}

export function buildMailtoUrl(input: ProblemReportInput): string {
  const subject = encodeURIComponent(buildProblemReportTitle(input));
  const body = encodeURIComponent(buildProblemReportBody(input));
  return `mailto:${PROBLEM_REPORT_EMAIL}?subject=${subject}&body=${body}`;
}

export function buildGitHubIssueUrl(input: ProblemReportInput): string {
  const repo = PROBLEM_REPORT_GITHUB_REPO.replace(/^https?:\/\/github\.com\//, '').replace(/\/$/, '');
  const params = new URLSearchParams({
    title: buildProblemReportTitle(input),
    body: buildProblemReportBody(input),
  });
  return `https://github.com/${repo}/issues/new?${params.toString()}`;
}
