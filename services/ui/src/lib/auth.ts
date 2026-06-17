const STORAGE_KEY = 'ferrum.passport';

export function loadStoredPassport(): string | null {
  try {
    return sessionStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

export function storePassport(jwt: string | null) {
  try {
    if (jwt) sessionStorage.setItem(STORAGE_KEY, jwt);
    else sessionStorage.removeItem(STORAGE_KEY);
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
  const params = new URLSearchParams(raw);
  return params.get('access_token');
}

export function authCallbackPath(): string {
  if (typeof window !== 'undefined' && window.location.pathname.startsWith('/ui')) {
    return '/ui/auth/callback';
  }
  return '/auth/callback';
}

export function buildBrokerLoginUrl(brokerLoginUrl: string): string {
  const returnUrl = `${window.location.origin}${authCallbackPath()}`;
  const join = brokerLoginUrl.includes('?') ? '&' : '?';
  return `${brokerLoginUrl}${join}return_url=${encodeURIComponent(returnUrl)}`;
}
