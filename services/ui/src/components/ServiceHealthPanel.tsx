import { useI18n } from '@/i18n/I18nProvider';
import { useServiceHealth } from '@/hooks/useServiceHealth';
import { ServiceHealthBadge } from './ServiceHealthBadge';

export function ServiceHealthPanel() {
  const { data: health, isLoading } = useServiceHealth();
  const { t } = useI18n();

  if (isLoading && !health) {
    return (
      <div className="flex flex-wrap gap-2">
        <ServiceHealthBadge status="degraded" label={t('health.checking')} />
      </div>
    );
  }

  return (
    <div className="flex flex-wrap gap-2">
      {(health ?? []).map((svc) => {
        const badgeStatus =
          svc.status === 'disabled'
            ? 'degraded'
            : svc.status === 'checking'
              ? 'degraded'
              : svc.status;
        return (
          <ServiceHealthBadge
            key={svc.id}
            status={badgeStatus}
            label={
              svc.status === 'disabled'
                ? `${t(svc.labelKey)} (${t('health.disabled')})`
                : t(svc.labelKey)
            }
          />
        );
      })}
    </div>
  );
}
