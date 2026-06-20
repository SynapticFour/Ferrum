import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';

export interface SanitizedAuth {
  mode?: string;
  require_auth?: boolean;
  broker_public_url?: string;
  broker_login_url?: string;
}

export interface SanitizedConfig {
  bind?: string;
  database?: { driver?: string; url_set?: boolean; run_migrations?: boolean; max_connections?: number };
  storage?: { backend?: string; s3_endpoint?: string; s3_bucket?: string };
  services?: {
    enable_drs?: boolean;
    enable_wes?: boolean;
    enable_tes?: boolean;
    enable_trs?: boolean;
    enable_beacon?: boolean;
    enable_passports?: boolean;
    enable_crypt4gh?: boolean;
    /** Gateway has Crypt4GH key dir — ingest encrypt-on-upload will work. */
    crypt4gh_ingest_ready?: boolean;
  };
  compute?: { tes_backend?: string; wes_trs_auto_register?: boolean };
  discovery?: {
    enabled?: boolean;
    auto_register?: boolean;
    service_registry_url?: string;
    registration_base_url?: string;
  };
  auth?: SanitizedAuth;
  deployment_mode?: string;
  message?: string;
}

export function useAdminConfig() {
  return useQuery({
    queryKey: ['admin', 'config'],
    queryFn: () => apiGet<SanitizedConfig>('/admin/config'),
    staleTime: 60_000,
    retry: false,
  });
}

export function isNoopExecutor(config?: SanitizedConfig | null): boolean {
  if (!config) return false;
  const backend = config.compute?.tes_backend?.toLowerCase();
  return backend === 'noop';
}

export function isCrypt4ghIngestReady(config?: SanitizedConfig | null): boolean {
  return config?.services?.crypt4gh_ingest_ready === true;
}

/** i18n key for why encrypt-on-upload is disabled on this deployment. */
export function crypt4ghUnavailableMessageKey(
  config?: SanitizedConfig | null,
): 'data.encryptUnavailableRemote' | 'data.encryptUnavailableLocal' {
  if (config?.auth?.require_auth) return 'data.encryptUnavailableRemote';
  return 'data.encryptUnavailableLocal';
}
