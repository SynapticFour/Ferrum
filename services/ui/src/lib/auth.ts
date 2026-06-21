const STORAGE_KEY = 'ferrum.passport';

export function loadStoredPassport(): string | null {
  try {
    return sessionStorage.getItem(STORAGE_KEY) ?? localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

export function storePassport(jwt: string | null) {
  try {
    if (jwt) {
      sessionStorage.setItem(STORAGE_KEY, jwt);
      localStorage.setItem(STORAGE_KEY, jwt);
    } else {
      sessionStorage.removeItem(STORAGE_KEY);
      localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    /* private browsing */
  }
}

export function decodeJwtPayload(jwt: string): Record<string, unknown> | null {
  const parts = jwt.split('.');
  if (parts.length < 2) return null;
  try {
    const b64 = parts[1].replace(/-/g, '+').replace(/_/g, '/');
    const json = atob(b64.padEnd(b64.length + ((4 - (b64.length % 4)) % 4), '='));
    return JSON.parse(json) as Record<string, unknown>;
  } catch {
    return null;
  }
}

/** True when JWT `exp` is in the past (30s skew). */
export function isPassportExpired(jwt: string | null | undefined): boolean {
  if (!jwt) return true;
  const claims = decodeJwtPayload(jwt);
  const exp = claims?.exp;
  if (typeof exp !== 'number') return false;
  return exp * 1000 <= Date.now() + 30_000;
}

export function passportExpiresAt(jwt: string | null | undefined): Date | null {
  if (!jwt) return null;
  const claims = decodeJwtPayload(jwt);
  const exp = claims?.exp;
  if (typeof exp !== 'number') return null;
  return new Date(exp * 1000);
}

export function parseTokenFromLocationHash(hash: string): string | null {
  const raw = hash.startsWith('#') ? hash.slice(1) : hash;
  if (!raw) return null;
  const params = new URLSearchParams(raw);
  const fromParams = params.get('access_token');
  if (fromParams) return fromParams;
  const match = raw.match(/(?:^|[&#])access_token=([^&#]+)/);
  if (!match?.[1]) return null;
  try {
    return decodeURIComponent(match[1]);
  } catch {
    return match[1];
  }
}

/** OAuth return path — must match TanStack Router basepath (Vite `base`). */
export function authCallbackPath(): string {
  const base = (import.meta.env.BASE_URL ?? '/').replace(/\/$/, '');
  return base ? `${base}/auth/callback` : '/auth/callback';
}

export function buildBrokerLoginUrl(brokerLoginUrl: string): string {
  const returnUrl = `${window.location.origin}${authCallbackPath()}`;
  const join = brokerLoginUrl.includes('?') ? '&' : '?';
  return `${brokerLoginUrl}${join}return_url=${encodeURIComponent(returnUrl)}`;
}
