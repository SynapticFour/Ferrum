import type { SanitizedConfig } from '@/hooks/useAdminConfig';
import { isNoopExecutor } from '@/hooks/useAdminConfig';

/** Fly hosted pilot: external auth + noop TES (API-only workflow runs). */
export function isFlyPilot(config?: SanitizedConfig | null): boolean {
  if (!config) return false;
  return Boolean(config.auth?.require_auth) && isNoopExecutor(config);
}

/** Pick operator-local vs hosted-pilot empty-state copy. */
export function pickSeedHintKey(config: SanitizedConfig | null | undefined, localKey: string): string {
  return isFlyPilot(config) ? 'pilot.remoteSeedHint' : localKey;
}
