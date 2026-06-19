import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { apiGet, apiPost } from '@/api/client';
import { useI18n } from '@/i18n/I18nProvider';
import { Users, Loader2 } from 'lucide-react';

interface WorkspaceMember {
  workspace_id: string;
  sub: string;
  role: string;
  invited_by: string;
  joined_at?: string;
}

export function WorkspaceMembersPanel({ workspaceId }: { workspaceId: string }) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [sub, setSub] = useState('');
  const [role, setRole] = useState('viewer');

  const { data: members, isLoading } = useQuery({
    queryKey: ['workspace', workspaceId, 'members'],
    queryFn: () =>
      apiGet<WorkspaceMember[]>(`/workspaces/v1/workspaces/${encodeURIComponent(workspaceId)}/members`),
    enabled: !!workspaceId,
  });

  const addMember = useMutation({
    mutationFn: () =>
      apiPost(`/workspaces/v1/workspaces/${encodeURIComponent(workspaceId)}/members`, { sub: sub.trim(), role }),
    onSuccess: () => {
      setSub('');
      void qc.invalidateQueries({ queryKey: ['workspace', workspaceId, 'members'] });
    },
  });

  const list = Array.isArray(members) ? members : [];

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Users className="h-4 w-4" />
          {t('workspace.membersTitle')}
        </CardTitle>
        <p className="text-sm text-muted-foreground">{t('workspace.membersHint')}</p>
      </CardHeader>
      <CardContent className="space-y-4">
        {isLoading && <p className="text-sm text-muted-foreground">{t('common.loading')}</p>}
        {list.length > 0 ? (
          <ul className="space-y-2 text-sm">
            {list.map((m) => (
              <li key={m.sub} className="flex justify-between rounded border border-border px-3 py-2">
                <span className="font-medium">{m.sub}</span>
                <span className="text-muted-foreground capitalize">{m.role}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-sm text-muted-foreground">{t('workspace.noMembers')}</p>
        )}
        <div className="grid gap-3 sm:grid-cols-3 pt-2 border-t border-border">
          <div className="space-y-1 sm:col-span-2">
            <Label htmlFor="member-sub">{t('workspace.memberSub')}</Label>
            <Input id="member-sub" value={sub} onChange={(e) => setSub(e.target.value)} placeholder={t('workspace.memberSubPlaceholder')} />
          </div>
          <div className="space-y-1">
            <Label>{t('workspace.memberRole')}</Label>
            <Select value={role} onValueChange={setRole}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="owner">{t('workspace.roleOwner')}</SelectItem>
                <SelectItem value="editor">{t('workspace.roleEditor')}</SelectItem>
                <SelectItem value="viewer">{t('workspace.roleViewer')}</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
        <Button
          type="button"
          disabled={!sub.trim() || addMember.isPending}
          onClick={() => addMember.mutate()}
          className="gap-2"
        >
          {addMember.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
          {t('workspace.addMember')}
        </Button>
      </CardContent>
    </Card>
  );
}
