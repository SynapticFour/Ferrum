import { useAuthConfig } from '@/hooks/useAuthConfig';
import { useAdminConfig } from '@/hooks/useAdminConfig';
import { useI18n } from '@/i18n/I18nProvider';
import { Info } from 'lucide-react';

/** Explains open demo vs authenticated admin — Settings is read-only until Phase 2. */
export function DemoModeNote() {
  const { t } = useI18n();
  const { data: authConfig } = useAuthConfig();
  const { data: config } = useAdminConfig();

  if (authConfig?.require_auth) {
    return (
      <div className="rounded-lg border border-border bg-muted/30 p-4 text-sm">
        <p className="font-medium">{t('settings.authenticatedMode')}</p>
        <p className="mt-1 text-muted-foreground">{t('settings.authenticatedModeHint')}</p>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-primary/30 bg-primary/5 p-4 text-sm flex gap-3">
      <Info className="h-5 w-5 shrink-0 text-primary mt-0.5" />
      <div>
        <p className="font-medium">{t('settings.demoMode')}</p>
        <p className="mt-1 text-muted-foreground">{t('settings.demoModeHint')}</p>
        {config?.deployment_mode && (
          <p className="mt-2 text-xs text-muted-foreground">
            {t('settings.deploymentMode')}: <code className="rounded bg-muted px-1">{config.deployment_mode}</code>
            {' · '}
            {t('settings.identity')}: <code className="rounded bg-muted px-1">demo-user</code>
          </p>
        )}
      </div>
    </div>
  );
}
