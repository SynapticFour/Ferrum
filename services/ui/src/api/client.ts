const BASE = '';

function getAuthHeader(): Record<string, string> {
  const jwt = (window as unknown as { __ferrumPassport?: string }).__ferrumPassport;
  if (jwt) return { Authorization: `Bearer ${jwt}` };
  return {};
}

export async function apiFetch<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...getAuthHeader(),
      ...options.headers,
    },
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(text || `HTTP ${res.status}`);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

export async function apiGet<T>(path: string): Promise<T> {
  return apiFetch<T>(path, { method: 'GET' });
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
  const res = await fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: {
      ...getAuthHeader(),
    },
    body: formData,
  });
  const text = await res.text();
  if (!res.ok) {
    let msg = text || `HTTP ${res.status}`;
    try {
      const j = JSON.parse(text) as { code?: string; message?: string; error?: string };
      if (typeof j.message === 'string') {
        msg = j.code ? `${j.code}: ${j.message}` : j.message;
      } else if (typeof j.error === 'string') {
        msg = j.error;
      }
    } catch {
      /* plain-text error body */
    }
    throw new Error(msg);
  }
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}
