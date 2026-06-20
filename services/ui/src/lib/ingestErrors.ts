/** Map raw API ingest errors to clearer user-facing text. */

export function extractIngestJobError(error: unknown): string | undefined {
  if (error == null) return undefined;
  if (typeof error === 'string') return error;
  if (typeof error === 'object' && error !== null) {
    const rec = error as Record<string, unknown>;
    if (typeof rec.message === 'string' && rec.message.trim()) return rec.message;
  }
  return undefined;
}

export function friendlyIngestError(
  raw: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
): string {
  const lower = raw.toLowerCase();
  if (lower.includes('failed to read stream') || lower.includes('upload stream interrupted')) {
    return t('data.uploadStreamError');
  }
  if (lower.includes('exceeds ingest.max_upload_bytes') || lower.includes('limit exceeded')) {
    return t('data.uploadTooLarge');
  }
  if (lower.includes('transfer_queued') || lower.includes('deferred on very low bandwidth')) {
    return t('data.uploadTransferQueued');
  }
  if (lower.includes('crypt4gh') || lower.includes('encrypt=true requires')) {
    return t('data.uploadEncryptUnavailable');
  }
  if (lower.includes('workspace_id requires') || lower.includes('not a workspace editor')) {
    return t('data.uploadWorkspaceForbidden');
  }
  if (lower.includes('unauthorized') || lower.includes('authentication')) {
    return t('common.sessionExpired');
  }
  if (raw.startsWith('validation_error:')) {
    return raw.replace(/^validation_error:\s*/i, '');
  }
  if (raw.startsWith('internal_error:')) {
    return raw.replace(/^internal_error:\s*/i, '');
  }
  return raw;
}
