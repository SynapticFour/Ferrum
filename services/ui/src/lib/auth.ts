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
