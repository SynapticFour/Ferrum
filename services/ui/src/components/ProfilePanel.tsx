import { useEffect, useState } from 'react';
import { useAuthStore } from '@/stores/auth';
import { useI18n } from '@/i18n/I18nProvider';
import { useAuthConfig } from '@/hooks/useAuthConfig';
import { Button } from '@/components/ui/button';
import {
  decodeJwtPayload,
  isPassportExpired,
  loadStoredPassport,
  passportExpiresAt,
} from '@/lib/auth';

interface VisaClaim {
  type?: string;
  value?: string;
  source?: string;
  asserted?: number;
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
  const setPassport = useAuthStore((s) => s.setPassport);
  const { data: authConfig } = useAuthConfig();
  const { t } = useI18n();
  const [copyState, setCopyState] = useState<'idle' | 'ok' | 'err'>('idle');

  useEffect(() => {
    const stored = loadStoredPassport();
    if (stored && !passportJwt && !isPassportExpired(stored)) {
      setPassport(stored);
    }
  }, [passportJwt, setPassport]);

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

  const token = passportJwt;
  const claims = decodeJwtPayload(token);
  if (!claims) {
    return <p className="text-sm text-destructive">{t('common.error')}</p>;
  }

  const sub = typeof claims.sub === 'string' ? claims.sub : undefined;
  const iss = typeof claims.iss === 'string' ? claims.iss : undefined;
  const visas = decodeVisas(claims.ga4gh_visa_v1 ?? claims.visas);
  const expiresAt = passportExpiresAt(token);

  async function copyApiToken() {
    try {
      await navigator.clipboard.writeText(token);
      setCopyState('ok');
      window.setTimeout(() => setCopyState('idle'), 2500);
    } catch {
      setCopyState('err');
      window.setTimeout(() => setCopyState('idle'), 2500);
    }
  }

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
      {expiresAt && (
        <div>
          <span className="font-medium text-muted-foreground">{t('settings.tokenExpires')}</span>
          <p className="text-xs">{expiresAt.toLocaleString()}</p>
        </div>
      )}
      <div className="rounded-md border border-border bg-muted/30 p-4 space-y-2">
        <p className="font-medium">{t('settings.apiTokenTitle')}</p>
        <p className="text-xs text-muted-foreground">{t('settings.apiTokenHint')}</p>
        <Button type="button" variant="outline" size="sm" onClick={() => void copyApiToken()}>
          {copyState === 'ok'
            ? t('settings.apiTokenCopied')
            : copyState === 'err'
              ? t('settings.apiTokenCopyFailed')
              : t('settings.copyApiToken')}
        </Button>
      </div>
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
