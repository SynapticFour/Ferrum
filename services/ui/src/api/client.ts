import { useAuthStore } from '@/stores/auth';
import { isPassportExpired } from '@/lib/auth';
import { fetchWithGatewayRetry } from '@/lib/apiFetchRetry';

const BASE = '';

export class ApiAuthError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly sessionExpired = false,
  ) {
    super(message);
    this.name = 'ApiAuthError';
  }
}

function getAuthHeader(): Record<string, string> {
  const jwt = useAuthStore.getState().passportJwt;
  if (jwt && !isPassportExpired(jwt)) return { Authorization: `Bearer ${jwt}` };
  return {};
}

function parseErrorMessage(text: string, status: number): { msg: string; sessionExpired: boolean } {
  let msg = text || `HTTP ${status}`;
  let sessionExpired = status === 401;
  try {
    const j = JSON.parse(text) as { code?: string; message?: string; error?: string };
    if (typeof j.message === 'string') {
      msg = j.code ? `${j.code}: ${j.message}` : j.message;
    } else if (typeof j.error === 'string') {
      msg = j.error;
    }
    if (/unauthorized|sign in|session may have expired|authentication required/i.test(msg)) {
      sessionExpired = true;
    }
  } catch {
    if (/Missing request extension|AuthClaims|unauthorized/i.test(text)) {
      sessionExpired = true;
      msg =
        'Your session has expired or is invalid. Please sign in again.';
    }
  }
  if (status === 502 || status === 503 || status === 504) {
    msg =
      'The server is still starting up or temporarily unavailable — wait a moment and try again.';
  }
  return { msg, sessionExpired };
}

function handleAuthFailure(sessionExpired: boolean) {
  if (sessionExpired) {
    useAuthStore.getState().setPassport(null);
  }
}

export async function apiFetch<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const jwt = useAuthStore.getState().passportJwt;
  if (jwt && isPassportExpired(jwt)) {
    useAuthStore.getState().setPassport(null);
    throw new ApiAuthError(
      'Your session has expired. Please sign in again.',
      401,
      true,
    );
  }

  const res = await fetchWithGatewayRetry(`${BASE}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...getAuthHeader(),
      ...options.headers,
    },
  });
  if (!res.ok) {
    const text = await res.text();
    const { msg, sessionExpired } = parseErrorMessage(text, res.status);
    handleAuthFailure(sessionExpired);
    throw new ApiAuthError(msg, res.status, sessionExpired);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

export async function apiGet<T>(path: string): Promise<T> {
  return apiFetch<T>(path, { method: 'GET' });
}

/** Plain-text GET (e.g. WES `/logs/stdout`). */
export async function apiGetText(path: string): Promise<string> {
  const jwt = useAuthStore.getState().passportJwt;
  if (jwt && isPassportExpired(jwt)) {
    useAuthStore.getState().setPassport(null);
    throw new ApiAuthError(
      'Your session has expired. Please sign in again.',
      401,
      true,
    );
  }

  const res = await fetchWithGatewayRetry(`${BASE}${path}`, {
    method: 'GET',
    headers: {
      ...getAuthHeader(),
    },
  });
  if (!res.ok) {
    const text = await res.text();
    const { msg, sessionExpired } = parseErrorMessage(text, res.status);
    handleAuthFailure(sessionExpired);
    throw new ApiAuthError(msg, res.status, sessionExpired);
  }
  return res.text();
}

export async function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, { method: 'POST', body: body ? JSON.stringify(body) : undefined });
}

export async function apiPut<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, { method: 'PUT', body: body ? JSON.stringify(body) : undefined });
}

export async function apiDelete(path: string): Promise<void> {
  return apiFetch(path, { method: 'DELETE' });
}

/** Multipart POST (e.g. `/api/v1/ingest/upload`). Do not set Content-Type — browser sets boundary. */
export async function apiPostFormData<T>(path: string, formData: FormData): Promise<T> {
  const jwt = useAuthStore.getState().passportJwt;
  if (jwt && isPassportExpired(jwt)) {
    useAuthStore.getState().setPassport(null);
    throw new ApiAuthError(
      'Your session has expired. Please sign in again.',
      401,
      true,
    );
  }

  const res = await fetchWithGatewayRetry(`${BASE}${path}`, {
    method: 'POST',
    headers: {
      ...getAuthHeader(),
    },
    body: formData,
  });
  const text = await res.text();
  if (!res.ok) {
    const { msg, sessionExpired } = parseErrorMessage(text, res.status);
    handleAuthFailure(sessionExpired);
    throw new ApiAuthError(msg, res.status, sessionExpired);
  }
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}
