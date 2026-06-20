import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiPost } from '@/api/client';
import { Search, Loader2, CheckCircle2, XCircle } from 'lucide-react';
import { ErrorWithReport } from '@/components/ErrorWithReport';
import { useI18n } from '@/i18n/I18nProvider';

interface VariantQueryResponse {
  meta: Record<string, unknown>;
  response: { exists?: boolean; count?: number };
}

interface BeaconQuerySnapshot {
  assemblyId: string;
  referenceName: string;
  start: string;
  referenceBases: string;
  alternateBases: string;
  federate: boolean;
}

const DEMO_PRESETS = [
  {
    id: 'local',
    labelKey: 'beacon.presetLocal',
    assemblyId: 'GRCh38',
    referenceName: '1',
    start: '1000',
    end: '1000',
    referenceBases: 'A',
    alternateBases: 'T',
  },
  {
    id: 'pasteur',
    labelKey: 'beacon.presetPasteur',
    assemblyId: 'GRCh37',
    referenceName: '22',
    start: '2000',
    end: '2000',
    referenceBases: 'T',
    alternateBases: 'G',
  },
] as const;

export function BeaconExplorer() {
  const { t } = useI18n();
  const [referenceName, setReferenceName] = useState('1');
  const [start, setStart] = useState('1000');
  const [end, setEnd] = useState('1000');
  const [referenceBases, setReferenceBases] = useState('A');
  const [alternateBases, setAlternateBases] = useState('T');
  const [assemblyId, setAssemblyId] = useState('GRCh38');
  const [federate, setFederate] = useState(false);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<VariantQueryResponse | null>(null);
  const [lastQuery, setLastQuery] = useState<BeaconQuerySnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);

  function applyPreset(preset: (typeof DEMO_PRESETS)[number]) {
    setAssemblyId(preset.assemblyId);
    setReferenceName(preset.referenceName);
    setStart(preset.start);
    setEnd(preset.end);
    setReferenceBases(preset.referenceBases);
    setAlternateBases(preset.alternateBases);
    setResult(null);
    setLastQuery(null);
    setError(null);
  }

  async function handleQuery() {
    const querySnapshot: BeaconQuerySnapshot = {
      assemblyId,
      referenceName,
      start,
      referenceBases,
      alternateBases,
      federate,
    };
    setLoading(true);
    setError(null);
    setResult(null);
    setLastQuery(null);
    try {
      const path = federate
        ? `/ga4gh/beacon/v2/g_variants?federate=true&referenceName=${encodeURIComponent(referenceName)}&start=${start}&referenceBases=${referenceBases}&alternateBases=${alternateBases}`
        : '/ga4gh/beacon/v2/g_variants/query';
      const body = federate
        ? undefined
        : {
            meta: { apiVersion: 'v2.0.0' },
            query: {
              requestParameters: {
                assemblyId,
                referenceName,
                start: parseInt(start, 10),
                end: end ? parseInt(end, 10) : parseInt(start, 10),
                referenceBases: referenceBases || undefined,
                alternateBases: alternateBases || undefined,
                requestedGranularity: 'boolean',
              },
            },
          };
      const res = await apiPost<VariantQueryResponse>(path, body);
      setResult(res);
      setLastQuery(querySnapshot);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  const exists = result?.response?.exists;
  const formChanged =
    lastQuery != null &&
    (assemblyId !== lastQuery.assemblyId ||
      referenceName !== lastQuery.referenceName ||
      start !== lastQuery.start ||
      referenceBases !== lastQuery.referenceBases ||
      alternateBases !== lastQuery.alternateBases ||
      federate !== lastQuery.federate);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t('beacon.title')}</h1>
        <p className="text-muted-foreground">{t('beacon.subtitle')}</p>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>{t('beacon.queryTitle')}</CardTitle>
          <p className="text-sm text-muted-foreground">{t('beacon.queryHint')}</p>
          <div className="flex flex-wrap gap-2 pt-2">
            {DEMO_PRESETS.map((p) => (
              <Button key={p.id} type="button" variant="outline" size="sm" onClick={() => applyPreset(p)}>
                {t(p.labelKey)}
              </Button>
            ))}
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-3">
            <div className="space-y-2">
              <Label htmlFor="assembly">{t('beacon.assembly')}</Label>
              <Input id="assembly" value={assemblyId} onChange={(e) => setAssemblyId(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="ref">{t('beacon.referenceName')}</Label>
              <Input id="ref" value={referenceName} onChange={(e) => setReferenceName(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="start">{t('beacon.start')}</Label>
              <Input id="start" type="number" value={start} onChange={(e) => setStart(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="end">{t('beacon.end')}</Label>
              <Input id="end" type="number" value={end} onChange={(e) => setEnd(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="refbases">{t('beacon.refBases')}</Label>
              <Input id="refbases" value={referenceBases} onChange={(e) => setReferenceBases(e.target.value)} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="altbases">{t('beacon.altBases')}</Label>
              <Input id="altbases" value={alternateBases} onChange={(e) => setAlternateBases(e.target.value)} />
            </div>
            <div className="flex items-end gap-2 pb-2">
              <label className="flex items-center gap-2 text-sm">
                <input type="checkbox" checked={federate} onChange={(e) => setFederate(e.target.checked)} />
                {t('beacon.federate')}
              </label>
            </div>
          </div>
          <Button type="button" onClick={handleQuery} disabled={loading} className="gap-2">
            {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
            {t('beacon.query')}
          </Button>
          {error && (
            <ErrorWithReport
              errorMessage={error}
              context="beacon-query"
              lastApi={{ method: 'POST', path: '/ga4gh/beacon/v2/g_variants' }}
              extra={
                lastQuery
                  ? {
                      assemblyId: lastQuery.assemblyId,
                      referenceName: lastQuery.referenceName,
                      start: lastQuery.start,
                      federate: lastQuery.federate,
                    }
                  : undefined
              }
            />
          )}
          {result && (
            <div
              className={
                exists
                  ? 'flex items-start gap-3 rounded-md border border-emerald-500/40 bg-emerald-500/10 px-4 py-3 text-sm'
                  : 'flex items-start gap-3 rounded-md border border-border bg-muted/30 px-4 py-3 text-sm'
              }
            >
              {exists ? (
                <CheckCircle2 className="h-5 w-5 text-emerald-500 shrink-0" />
              ) : (
                <XCircle className="h-5 w-5 text-muted-foreground shrink-0" />
              )}
              <div>
                <p className="font-medium">
                  {exists ? t('beacon.variantFound') : t('beacon.variantNotFound')}
                </p>
                {lastQuery && (
                  <p className="text-muted-foreground mt-1">
                    {lastQuery.federate ? t('beacon.federate') : lastQuery.assemblyId} · chr
                    {lastQuery.referenceName}:{lastQuery.start} {lastQuery.referenceBases}&gt;
                    {lastQuery.alternateBases}
                  </p>
                )}
                {formChanged && (
                  <p className="text-muted-foreground mt-1 text-xs italic">{t('beacon.formChanged')}</p>
                )}
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
