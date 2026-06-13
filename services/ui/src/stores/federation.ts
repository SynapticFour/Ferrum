const STORAGE_KEY = 'ferrum.federation.prefs';

export interface FederationPrefs {
  registryUrl: string;
  publicBaseUrl: string;
  nodeIdPrefix: string;
  organizationName: string;
}

const defaults: FederationPrefs = {
  registryUrl: '',
  publicBaseUrl: typeof window !== 'undefined' ? window.location.origin : 'http://127.0.0.1:8080',
  nodeIdPrefix: 'org.ferrum.laptop',
  organizationName: 'My Ferrum Node',
};

export function loadFederationPrefs(): FederationPrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...defaults };
    return { ...defaults, ...JSON.parse(raw) };
  } catch {
    return { ...defaults };
  }
}

export function saveFederationPrefs(prefs: FederationPrefs): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
}
