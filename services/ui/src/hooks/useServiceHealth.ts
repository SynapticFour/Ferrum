import { useQuery } from '@tanstack/react-query';
import { useAuthStore } from '@/stores/auth';
import { useAdminConfig } from './useAdminConfig';

export type HealthStatus = 'up' | 'degraded' | 'down' | 'disabled' | 'checking';

export interface ServiceHealth {
  id: string;
  labelKey: string;
  status: HealthStatus;
}

async function probe(path: string): Promise<boolean> {
  const jwt = useAuthStore.getState().passportJwt;
  try {
    const res = await fetch(path, {
      method: 'GET',
      headers: jwt ? { Authorization: `Bearer ${jwt}` } : {},
    });
    return res.ok;
  } catch {
    return false;
  }
}

export function useServiceHealth() {
  const { data: config, isLoading: configLoading } = useAdminConfig();

  return useQuery({
    queryKey: ['system', 'health', config?.services],
    queryFn: async (): Promise<ServiceHealth[]> => {
      const services = config?.services;
      const checks: Array<{ id: string; labelKey: string; enabled: boolean; path: string }> = [
        { id: 'gateway', labelKey: 'health.gateway', enabled: true, path: '/health' },
        { id: 'drs', labelKey: 'health.drs', enabled: !!services?.enable_drs, path: '/ga4gh/drs/v1/service-info' },
        { id: 'wes', labelKey: 'health.wes', enabled: !!services?.enable_wes, path: '/ga4gh/wes/v1/service-info' },
        { id: 'tes', labelKey: 'health.tes', enabled: !!services?.enable_tes, path: '/ga4gh/wes/v1/service-info' },
        { id: 'trs', labelKey: 'health.trs', enabled: !!services?.enable_trs, path: '/ga4gh/trs/v2/service-info' },
        { id: 'beacon', labelKey: 'health.beacon', enabled: !!services?.enable_beacon, path: '/ga4gh/beacon/v2/service-info' },
      ];

      const results = await Promise.all(
        checks.map(async (c) => {
          if (!c.enabled) {
            return { id: c.id, labelKey: c.labelKey, status: 'disabled' as HealthStatus };
          }
          const ok = await probe(c.path);
          return {
            id: c.id,
            labelKey: c.labelKey,
            status: (ok ? 'up' : 'down') as HealthStatus,
          };
        }),
      );
      return results;
    },
    enabled: !configLoading && !!config && !('message' in config && config.message),
    refetchInterval: 60_000,
    staleTime: 30_000,
    retry: false,
  });
}
