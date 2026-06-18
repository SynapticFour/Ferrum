import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { AlertCircle, Laptop, Server, Network } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface AdminConfig {
  deployment_mode?: string;
  services?: {
    enable_wes?: boolean;
    enable_trs?: boolean;
    enable_beacon?: boolean;
    enable_drs?: boolean;
  };
  discovery?: {
    enabled?: boolean;
    auto_register?: boolean;
    service_registry_url?: string;
  };
  message?: string;
}

const MODE_COPY: Record<
  string,
  { title: string; description: string; icon: typeof Laptop; tone: string }
> = {
  offline: {
    title: 'Edge mode',
    description:
      'Local SQLite + file storage on edge hardware (e.g. Raspberry Pi). DRS ingest, Beacon, and htsget work out of the box. TRS/WES need a full gateway or federation.',
    icon: Laptop,
    tone: 'border-amber-500/40 bg-amber-500/10 text-amber-900 dark:text-amber-100',
  },
  connected: {
    title: 'Edge + remote services',
    description:
      'Offline-first storage with WES/TRS enabled. Join federation in Settings → Federation to browse remote tools.',
    icon: Network,
    tone: 'border-primary/40 bg-primary/5 text-foreground',
  },
  full: {
    title: 'Full gateway',
    description:
      'PostgreSQL-backed services with TRS, WES, and optional auto-registration to the GA4GH service registry.',
    icon: Server,
    tone: 'border-emerald-500/40 bg-emerald-500/10 text-foreground',
  },
};

export function DeploymentModeBanner() {
  const { data: config } = useQuery({
    queryKey: ['admin', 'config', 'banner'],
    queryFn: () => apiGet<AdminConfig>('/admin/config'),
    retry: false,
    staleTime: 60_000,
  });

  if (!config || config.message) return null;

  const mode = config.deployment_mode ?? 'full';
  const copy = MODE_COPY[mode] ?? MODE_COPY.full;
  const Icon = copy.icon;
  const showEdgeHint = mode === 'offline';

  return (
    <section
      className={cn(
        'rounded-xl border p-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between',
        copy.tone
      )}
    >
      <div className="flex gap-3">
        <Icon className="h-5 w-5 shrink-0 mt-0.5" />
        <div>
          <p className="font-semibold">{copy.title}</p>
          <p className="text-sm opacity-90 mt-0.5">{copy.description}</p>
          {showEdgeHint && (
            <p className="text-xs mt-2 flex items-start gap-1.5 opacity-80">
              <AlertCircle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
              Run <code className="rounded bg-background/60 px-1">ferrum demo seed</code> in another
              terminal while the gateway is up to load demo DRS + Beacon data.
            </p>
          )}
        </div>
      </div>
      <div className="flex flex-wrap gap-2 shrink-0">
        {mode === 'offline' && (
          <Button asChild variant="secondary" size="sm">
            <Link to={'/settings' as any} hash="federation">
              Join federation
            </Link>
          </Button>
        )}
        <Button asChild variant="outline" size="sm">
          <Link to={'/settings' as any}>Settings</Link>
        </Button>
      </div>
    </section>
  );
}
