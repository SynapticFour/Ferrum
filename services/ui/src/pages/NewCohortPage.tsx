import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiPost } from '@/api/client';
import { ArrowLeft } from 'lucide-react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useI18n } from '@/i18n/I18nProvider';

const COHORTS_BASE = '/cohorts/v1';

type CreateResponse = {
  id: string;
  name: string;
  description: string | null;
};

export function NewCohortPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [workspaceId, setWorkspaceId] = useState('demo-workspace-01');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const res = await apiPost<CreateResponse>(`${COHORTS_BASE}/cohorts`, {
        name: name || t('cohortNew.unnamed'),
        description: description || null,
        workspace_id: workspaceId.trim() || null,
        tags: [],
        filter_criteria: {},
      });
      void (navigate as (opts: { to: string }) => void)({ to: `/cohorts/${res.id}` });
    } catch (err) {
      setError(err instanceof Error ? err.message : t('cohortNew.failed'));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link to={"/cohorts" as any}>
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <h1 className="text-3xl font-bold tracking-tight">{t('cohortNew.title')}</h1>
      </div>
      <Card className="max-w-lg">
        <CardHeader>
          <CardTitle>{t('cohortNew.createTitle')}</CardTitle>
          <p className="text-sm text-muted-foreground">{t('cohortNew.hint')}</p>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <Label htmlFor="name">{t('workspace.nameLabel')}</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t('cohortNew.namePlaceholder')}
                className="mt-1"
              />
            </div>
            <div>
              <Label htmlFor="workspace">{t('cohortNew.workspaceLabel')}</Label>
              <Input
                id="workspace"
                value={workspaceId}
                onChange={(e) => setWorkspaceId(e.target.value)}
                placeholder={t('cohortNew.workspacePlaceholder')}
                className="mt-1"
              />
            </div>
            <div>
              <Label htmlFor="description">{t('workspace.descLabel')} ({t('common.optional')})</Label>
              <Input
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t('cohortNew.descPlaceholder')}
                className="mt-1"
              />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button type="submit" disabled={submitting}>
              {submitting ? t('cohortNew.creating') : t('cohortNew.create')}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
