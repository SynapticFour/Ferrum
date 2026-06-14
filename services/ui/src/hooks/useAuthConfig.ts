import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';

export interface AuthConfig {
  mode?: string;
  require_auth?: boolean;
  broker_public_url?: string;
  broker_login_url?: string;
}

interface AdminConfigResponse {
  auth?: AuthConfig;
}

export function useAuthConfig() {
  return useQuery({
    queryKey: ['admin', 'config', 'auth'],
    queryFn: async () => {
      const cfg = await apiGet<AdminConfigResponse>('/admin/config');
      return cfg.auth ?? {};
    },
    staleTime: 60_000,
    retry: 1,
  });
}
