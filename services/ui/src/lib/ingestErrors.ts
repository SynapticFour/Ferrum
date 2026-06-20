/** Map raw API ingest errors to clearer user-facing text. */
export function friendlyIngestError(
  raw: string,
  t: (key: string) => string,
): string {
  const lower = raw.toLowerCase();
  if (lower.includes('failed to read stream') || lower.includes('upload stream interrupted')) {
    return t('data.uploadStreamError');
  }
  if (lower.includes('exceeds ingest.max_upload_bytes') || lower.includes('limit exceeded')) {
    return t('data.uploadTooLarge');
  }
  if (raw.startsWith('validation_error:')) {
    return raw.replace(/^validation_error:\s*/i, '');
  }
  if (raw.startsWith('internal_error:')) {
    return raw.replace(/^internal_error:\s*/i, '');
  }
  return raw;
}
