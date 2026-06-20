const RETRY_STATUSES = new Set([502, 503, 504]);
const RETRY_ATTEMPTS = 3;
const RETRY_DELAY_MS = 2500;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

/** Fetch with short retries for gateway cold-start / proxy errors. */
export async function fetchWithGatewayRetry(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  let last: Response | undefined;
  for (let attempt = 0; attempt < RETRY_ATTEMPTS; attempt++) {
    const res = await fetch(input, init);
    if (!RETRY_STATUSES.has(res.status) || attempt === RETRY_ATTEMPTS - 1) {
      return res;
    }
    last = res;
    await sleep(RETRY_DELAY_MS * (attempt + 1));
  }
  return last!;
}
