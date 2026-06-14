import { useAuthStore } from '@/stores/auth';
import { useI18n } from '@/i18n/I18nProvider';
import { useAuthConfig } from '@/hooks/useAuthConfig';

interface VisaClaim {
  type?: string;
  value?: string;
  source?: string;
  asserted?: number;
}

function decodeJwtPayload(jwt: string): Record<string, unknown> | null {
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

function decodeVisas(raw: unknown): VisaClaim[] {
  if (!Array.isArray(raw)) return [];
  const out: VisaClaim[] = [];
  for (const item of raw) {
    if (typeof item === 'object' && item !== null && 'type' in item) {
      out.push(item as VisaClaim);
      continue;
    }
    if (typeof item === 'string') {
      const payload = decodeJwtPayload(item);
      if (payload && typeof payload.type === 'string') {
        out.push({
          type: payload.type as string,
          value: typeof payload.value === 'string' ? payload.value : undefined,
          source: typeof payload.source === 'string' ? payload.source : undefined,
        });
      }
    }
  }
  return out;
}

export function ProfilePanel() {
  const passportJwt = useAuthStore((s) => s.passportJwt);
  const { data: authConfig } = useAuthConfig();
  const { t } = useI18n();

  if (!authConfig?.require_auth) {
    return (
      <p className="text-sm text-muted-foreground">{t('settings.authDisabled')}</p>
    );
  }

  if (!passportJwt) {
    return (
      <div className="rounded-md border border-border bg-muted/30 p-4 text-sm">
        <p className="font-medium">{t('common.noSession')}</p>
        <p className="mt-1 text-muted-foreground">{t('settings.notSignedIn')}</p>
      </div>
    );
  }

  const claims = decodeJwtPayload(passportJwt);
  if (!claims) {
    return <p className="text-sm text-destructive">{t('common.error')}</p>;
  }

  const sub = typeof claims.sub === 'string' ? claims.sub : undefined;
  const iss = typeof claims.iss === 'string' ? claims.iss : undefined;
  const visas = decodeVisas(claims.ga4gh_visa_v1 ?? claims.visas);

  return (
    <div className="space-y-4 text-sm">
      {sub && (
        <div>
          <span className="font-medium text-muted-foreground">{t('settings.signedInAs')}</span>
          <p className="font-mono break-all">{sub}</p>
        </div>
      )}
      {iss && (
        <div>
          <span className="font-medium text-muted-foreground">{t('settings.issuer')}</span>
          <p className="font-mono break-all text-xs">{iss}</p>
        </div>
      )}
      <div>
        <span className="font-medium text-muted-foreground">{t('settings.visas')}</span>
        {visas.length === 0 ? (
          <p className="mt-1 text-muted-foreground">{t('settings.noVisas')}</p>
        ) : (
          <ul className="mt-2 space-y-2">
            {visas.map((v, i) => (
              <li key={i} className="rounded-md border border-border px-3 py-2">
                <p className="font-medium">{v.type ?? '—'}</p>
                {v.value && <p className="text-xs text-muted-foreground break-all">{v.value}</p>}
              </li>
            ))}
          </ul>
        )}
      </div>
      <details>
        <summary className="cursor-pointer text-muted-foreground">{t('settings.rawClaims')}</summary>
        <pre className="mt-2 max-h-48 overflow-auto rounded-md bg-muted p-3 text-xs">
          {JSON.stringify(claims, null, 2)}
        </pre>
      </details>
    </div>
  );
}
