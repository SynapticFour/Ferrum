import { Link, useNavigate } from '@tanstack/react-router';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useState } from 'react';
import { apiPost } from '@/api/client';
import { ArrowLeft } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';

export function NewWorkspacePage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [slug, setSlug] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const ws = await apiPost<{ id: string }>('/workspaces/v1/workspaces', {
        name: name.trim(),
        description: description.trim() || undefined,
        slug: slug.trim() || undefined,
      });
      if (ws?.id) {
        void (navigate as (opts: { to: string }) => void)({ to: `/workspaces/${ws.id}` });
      } else {
        setError(t('workspaceNew.invalidResponse'));
      }
    } catch (err) {
      let msg = err instanceof Error ? err.message : t('workspaceNew.failed');
      try {
        const parsed = JSON.parse(msg);
        if (parsed.message) msg = parsed.message;
      } catch {
        /* keep msg */
      }
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon" asChild>
          <Link to={'/workspaces' as any}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <h1 className="text-2xl font-bold">{t('workspaceNew.title')}</h1>
      </div>
      <Card className="max-w-md">
        <CardHeader>
          <CardTitle>{t('workspaceNew.createTitle')}</CardTitle>
          <p className="text-sm text-muted-foreground mt-1">{t('workspaceNew.hint')}</p>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <Label htmlFor="name">{t('workspace.nameLabel')}</Label>
              <Input id="name" value={name} onChange={(e) => setName(e.target.value)} placeholder={t('workspaceNew.namePlaceholder')} required />
            </div>
            <div>
              <Label htmlFor="description">{t('workspace.descLabel')}</Label>
              <Input id="description" value={description} onChange={(e) => setDescription(e.target.value)} placeholder={t('workspaceNew.descPlaceholder')} />
            </div>
            <div>
              <Label htmlFor="slug">{t('workspaceNew.slugLabel')}</Label>
              <Input id="slug" value={slug} onChange={(e) => setSlug(e.target.value)} placeholder={t('workspaceNew.slugPlaceholder')} />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button type="submit" disabled={loading}>
              {loading ? t('workspaceNew.creating') : t('workspaceNew.create')}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
