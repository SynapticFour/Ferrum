import { useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiGet, apiPost } from '@/api/client';
import { useI18n } from '@/i18n/I18nProvider';
import { Shield, Loader2, Ban } from 'lucide-react';

interface SecurityEvent {
  id: string;
  event_type: string;
  severity: string;
  sub?: string;
  ip_address?: string;
  resource_id?: string;
  occurred_at?: string;
}

interface EventsResponse {
  events: SecurityEvent[];
}

export function SecurityEventsPanel() {
  const { t } = useI18n();
  const [severity, setSeverity] = useState('');
  const [revokeJti, setRevokeJti] = useState('');
  const [revokeMsg, setRevokeMsg] = useState<string | null>(null);

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['admin', 'security', 'events', severity],
    queryFn: () => {
      const q = severity ? `?severity=${encodeURIComponent(severity)}&limit=50` : '?limit=50';
      return apiGet<EventsResponse>(`/admin/security/events${q}`);
    },
    retry: false,
  });

  const revoke = useMutation({
    mutationFn: () => apiPost<{ revoked: boolean }>('/admin/tokens/revoke', { jti: revokeJti.trim() }),
    onSuccess: (res) => {
      setRevokeMsg(res.revoked ? t('security.revoked') : t('security.revokeFailed'));
      void refetch();
    },
    onError: (e: Error) => setRevokeMsg(e.message),
  });

  const events = data?.events ?? [];
  const forbidden = error && String(error).includes('403');

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Shield className="h-4 w-4" />
            {t('security.eventsTitle')}
          </CardTitle>
          <p className="text-sm text-muted-foreground">{t('security.eventsHint')}</p>
        </CardHeader>
        <CardContent className="space-y-4">
          {forbidden && (
            <p className="text-sm text-amber-600 dark:text-amber-400">{t('security.adminRequired')}</p>
          )}
          <div className="flex flex-wrap gap-2 items-end">
            <div className="space-y-1">
              <Label htmlFor="sev-filter">{t('security.severityFilter')}</Label>
              <Input
                id="sev-filter"
                value={severity}
                onChange={(e) => setSeverity(e.target.value)}
                placeholder="info, warning, critical"
                className="w-40"
              />
            </div>
            <Button type="button" variant="outline" onClick={() => void refetch()}>
              {t('security.refresh')}
            </Button>
          </div>
          {isLoading && <p className="text-sm text-muted-foreground">{t('common.loading')}</p>}
          {!isLoading && !forbidden && events.length === 0 && (
            <p className="text-sm text-muted-foreground">{t('security.noEvents')}</p>
          )}
          {events.length > 0 && (
            <div className="rounded-md border overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b bg-muted/50">
                    <th className="p-2 text-left">{t('security.when')}</th>
                    <th className="p-2 text-left">{t('security.type')}</th>
                    <th className="p-2 text-left">{t('security.severity')}</th>
                    <th className="p-2 text-left">sub</th>
                  </tr>
                </thead>
                <tbody>
                  {events.map((e) => (
                    <tr key={e.id} className="border-b last:border-0">
                      <td className="p-2 text-muted-foreground whitespace-nowrap">
                        {e.occurred_at ? new Date(e.occurred_at).toLocaleString() : '—'}
                      </td>
                      <td className="p-2 font-mono text-xs">{e.event_type}</td>
                      <td className="p-2">{e.severity}</td>
                      <td className="p-2">{e.sub ?? '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Ban className="h-4 w-4" />
            {t('security.revokeTitle')}
          </CardTitle>
          <p className="text-sm text-muted-foreground">{t('security.revokeHint')}</p>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-2 items-end">
          <div className="space-y-1 flex-1 min-w-[200px]">
            <Label htmlFor="jti">{t('security.jtiLabel')}</Label>
            <Input id="jti" value={revokeJti} onChange={(e) => setRevokeJti(e.target.value)} placeholder="token-jti-uuid" />
          </div>
          <Button
            type="button"
            disabled={!revokeJti.trim() || revoke.isPending}
            onClick={() => revoke.mutate()}
            className="gap-2"
          >
            {revoke.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
            {t('security.revoke')}
          </Button>
          {revokeMsg && <p className="w-full text-sm text-muted-foreground">{revokeMsg}</p>}
        </CardContent>
      </Card>
    </div>
  );
}
