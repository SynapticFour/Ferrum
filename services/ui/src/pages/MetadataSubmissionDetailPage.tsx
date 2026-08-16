import { Link, useParams } from '@tanstack/react-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  flattenJsonDiff,
  getSubmission,
  getVersion,
  issuesFromUnknown,
  listVersions,
  putSubmission,
} from '@/api/metadata';
import { useI18n } from '@/i18n/I18nProvider';
import { ApiAuthError } from '@/api/client';

export function MetadataSubmissionDetailPage() {
  const { t } = useI18n();
  const params = useParams({ strict: false }) as { alias?: string };
  const alias = params.alias ?? '';
  const qc = useQueryClient();
  const [draft, setDraft] = useState<string | null>(null);
  const [left, setLeft] = useState<number | null>(null);
  const [right, setRight] = useState<number | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const submission = useQuery({
    queryKey: ['metadata', 'submission', alias],
    queryFn: () => getSubmission(alias),
    enabled: !!alias,
    retry: false,
  });
  const versions = useQuery({
    queryKey: ['metadata', 'versions', alias],
    queryFn: () => listVersions(alias),
    enabled: !!alias,
    retry: false,
  });

  const leftDoc = useQuery({
    queryKey: ['metadata', 'version', alias, left],
    queryFn: () => getVersion(alias, left!),
    enabled: !!alias && left != null,
  });
  const rightDoc = useQuery({
    queryKey: ['metadata', 'version', alias, right],
    queryFn: () => getVersion(alias, right!),
    enabled: !!alias && right != null,
  });

  const diff = useMemo(() => {
    if (!leftDoc.data || !rightDoc.data) return [];
    return flattenJsonDiff(leftDoc.data.document, rightDoc.data.document);
  }, [leftDoc.data, rightDoc.data]);

  const save = useMutation({
    mutationFn: async () => {
      if (!submission.data || draft == null) return;
      const document = JSON.parse(draft) as unknown;
      return putSubmission(alias, document, submission.data.version);
    },
    onSuccess: () => {
      setSaveError(null);
      void qc.invalidateQueries({ queryKey: ['metadata'] });
    },
    onError: (err) => {
      const issues = issuesFromUnknown(err);
      if (issues.length) {
        setSaveError(issues.map((i) => `${i.path ?? '/'}: ${i.message ?? ''}`).join('\n'));
      } else {
        setSaveError(err instanceof ApiAuthError ? err.message : String(err));
      }
    },
  });

  if (!alias) return <p className="text-destructive">{t('metadataDetail.noAlias')}</p>;
  if (submission.isLoading) {
    return <p className="text-muted-foreground">{t('metadataDetail.loading')}</p>;
  }
  if (submission.error) {
    return <p className="text-destructive">{String(submission.error)}</p>;
  }
  const doc = submission.data;
  if (!doc) return null;
  const text = draft ?? JSON.stringify(doc.document, null, 2);

  return (
    <div className="space-y-6">
      <div>
        <Button asChild variant="ghost" size="sm" className="-ml-2 mb-2">
          <Link to={'/metadata' as any}>{t('metadataDetail.back')}</Link>
        </Button>
        <h1 className="text-3xl font-bold tracking-tight font-mono">{doc.alias}</h1>
        <p className="text-muted-foreground flex flex-wrap gap-2 items-center mt-1">
          <Badge variant="secondary">{doc.profile}</Badge>
          <span>v{doc.version}</span>
        </p>
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>{t('metadataDetail.document')}</CardTitle>
          <Button
            size="sm"
            disabled={save.isPending}
            onClick={() => save.mutate()}
          >
            {t('metadataDetail.save')}
          </Button>
        </CardHeader>
        <CardContent className="space-y-3">
          <textarea
            className="w-full min-h-[240px] font-mono text-xs rounded-md border bg-background p-3"
            value={text}
            onChange={(e) => setDraft(e.target.value)}
            spellCheck={false}
          />
          {saveError && (
            <pre className="text-xs text-destructive whitespace-pre-wrap bg-destructive/5 rounded p-3">
              {saveError}
            </pre>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t('metadataDetail.diffTitle')}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex flex-wrap gap-3 text-sm">
            <label className="flex items-center gap-2">
              {t('metadataDetail.versionA')}
              <select
                className="border rounded px-2 py-1 bg-background"
                value={left ?? ''}
                onChange={(e) => setLeft(e.target.value ? Number(e.target.value) : null)}
              >
                <option value="">{t('metadataDetail.pick')}</option>
                {(versions.data?.items ?? []).map((v) => (
                  <option key={v.version} value={v.version}>
                    v{v.version}
                    {v.is_current ? ' *' : ''}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-2">
              {t('metadataDetail.versionB')}
              <select
                className="border rounded px-2 py-1 bg-background"
                value={right ?? ''}
                onChange={(e) => setRight(e.target.value ? Number(e.target.value) : null)}
              >
                <option value="">{t('metadataDetail.pick')}</option>
                {(versions.data?.items ?? []).map((v) => (
                  <option key={`r-${v.version}`} value={v.version}>
                    v{v.version}
                    {v.is_current ? ' *' : ''}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {left != null && right != null && diff.length === 0 && (
            <p className="text-sm text-muted-foreground">{t('metadataDetail.diffEmpty')}</p>
          )}
          {diff.length > 0 && (
            <div className="overflow-auto text-xs font-mono space-y-2">
              {diff.map((row) => (
                <div key={row.path} className="border rounded p-2">
                  <p className="font-semibold">{row.path}</p>
                  <p className="text-destructive">- {row.left}</p>
                  <p className="text-emerald-700 dark:text-emerald-400">+ {row.right}</p>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
