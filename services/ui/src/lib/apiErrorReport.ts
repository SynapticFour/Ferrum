import { ApiAuthError } from '@/api/client';
import type { ProblemReportApiHint } from '@/lib/reportProblem';

/** Build lastApi hint for problem reports from a caught API error. */
export function lastApiFromError(
  error: unknown,
  method: string,
  path: string,
): ProblemReportApiHint {
  return {
    method,
    path,
    status: error instanceof ApiAuthError ? error.status : undefined,
  };
}

export function errorMessageFromUnknown(error: unknown, fallback: string): string {
  if (error instanceof ApiAuthError) return error.message;
  if (error instanceof Error && error.message) return error.message;
  return fallback;
}
