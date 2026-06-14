import { useEffect, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiPut } from '@/api/client';
import { useI18n } from '@/i18n/I18nProvider';
import { Settings, Loader2 } from 'lucide-react';

interface WorkspaceSettingsFormProps {
  workspaceId: string;
  name: string;
  description: string | null;
}

export function WorkspaceSettingsForm({ workspaceId, name, description }: WorkspaceSettingsFormProps) {
  const { t } = useI18n();
  const qc = useQueryClient();
  const [editName, setEditName] = useState(name);
  const [editDesc, setEditDesc] = useState(description ?? '');
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setEditName(name);
    setEditDesc(description ?? '');
  }, [name, description]);

  const save = useMutation({
    mutationFn: () =>
      apiPut(`/workspaces/v1/workspaces/${encodeURIComponent(workspaceId)}`, {
        name: editName.trim() || name,
        description: editDesc.trim() || null,
      }),
    onSuccess: () => {
      setSaved(true);
      void qc.invalidateQueries({ queryKey: ['workspace', workspaceId] });
      setTimeout(() => setSaved(false), 2000);
    },
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Settings className="h-4 w-4" />
          {t('workspace.settingsFormTitle')}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-1">
          <Label htmlFor="ws-name">{t('workspace.nameLabel')}</Label>
          <Input id="ws-name" value={editName} onChange={(e) => setEditName(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label htmlFor="ws-desc">{t('workspace.descLabel')}</Label>
          <Input id="ws-desc" value={editDesc} onChange={(e) => setEditDesc(e.target.value)} />
        </div>
        <Button type="button" disabled={save.isPending} onClick={() => save.mutate()} className="gap-2">
          {save.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
          {saved ? t('common.success') : t('common.save')}
        </Button>
      </CardContent>
    </Card>
  );
}
