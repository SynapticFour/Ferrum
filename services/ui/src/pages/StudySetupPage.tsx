import { useEffect, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiGet, apiPost } from '@/api/client';
import { useAuthConfig } from '@/hooks/useAuthConfig';
import { useAuthStore } from '@/stores/auth';
import { buildBrokerLoginUrl } from '@/lib/auth';
import { useI18n } from '@/i18n/I18nProvider';
import { ImportToDrsDialog } from '@/components/ImportToDrsDialog';
import { LinkWorkspaceDataDialog } from '@/components/LinkWorkspaceDataDialog';
import { RegisterToolDialog } from '@/components/RegisterToolDialog';
import { SampleSheetImportDialog } from '@/components/SampleSheetImportDialog';
import { RunCohortDialog } from '@/components/RunCohortDialog';
import { TRS_IMPORT_PRESETS } from '@/lib/trsCatalogs';
import {
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  Circle,
  Database,
  FolderOpen,
  LogIn,
  Play,
  Users,
  Wrench,
} from 'lucide-react';

const STORAGE_KEY = 'ferrum-study-setup-v1';
const COHORTS_BASE = '/cohorts/v1';

interface WizardState {
  step: number;
  workspaceId: string;
  cohortId: string;
  cohortName: string;
}

const DEFAULT_STATE: WizardState = {
  step: 1,
  workspaceId: 'demo-workspace-01',
  cohortId: 'demo-cohort-01',
  cohortName: 'Demo cohort',
};

function loadState(): WizardState {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_STATE, ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return DEFAULT_STATE;
}

function saveState(s: WizardState) {
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

const STEPS = [
  { id: 1, key: 'study.step1', icon: LogIn },
  { id: 2, key: 'study.step2', icon: FolderOpen },
  { id: 3, key: 'study.step3', icon: Database },
  { id: 4, key: 'study.step4', icon: Wrench },
  { id: 5, key: 'study.step5', icon: Users },
  { id: 6, key: 'study.step6', icon: Play },
  { id: 7, key: 'study.step7', icon: CheckCircle2 },
] as const;

export function StudySetupPage() {
  const { t } = useI18n();
  const { data: authConfig } = useAuthConfig();
  const passportJwt = useAuthStore((s) => s.passportJwt);
  const [state, setState] = useState<WizardState>(loadState);
  const [newCohortName, setNewCohortName] = useState('');
  const [creatingCohort, setCreatingCohort] = useState(false);
  const [runDialogOpen, setRunDialogOpen] = useState(false);
  const [registerPreset, setRegisterPreset] = useState(TRS_IMPORT_PRESETS[0] ? {
    name: t(TRS_IMPORT_PRESETS[0].nameKey),
    workflowUrl: TRS_IMPORT_PRESETS[0].workflowUrl,
    engineId: TRS_IMPORT_PRESETS[0].engineId,
    toolclass: TRS_IMPORT_PRESETS[0].toolclass,
  } : null);

  useEffect(() => {
    saveState(state);
  }, [state]);

  const { data: workspaces } = useQuery({
    queryKey: ['workspaces', 'mine'],
    queryFn: () => apiGet<Array<{ id: string; name: string }>>('/workspaces/v1/workspaces'),
  });

  const { data: runs } = useQuery({
    queryKey: ['wes', 'runs', 'study', state.workspaceId],
    queryFn: () =>
      apiGet<{ runs: Array<{ run_id: string; state?: string }> }>(
        `/ga4gh/wes/v1/runs?workspace_id=${encodeURIComponent(state.workspaceId)}&page_size=10`,
      ),
    enabled: state.step >= 7,
  });

  const wsList = Array.isArray(workspaces) ? workspaces : [];

  useEffect(() => {
    if (wsList.length === 0) return;
    const ids = new Set(wsList.map((w) => w.id));
    if (!ids.has(state.workspaceId)) {
      setState((s) => ({ ...s, workspaceId: wsList[0].id }));
    }
  }, [wsList, state.workspaceId]);

  const step = state.step;

  async function createCohort() {
    setCreatingCohort(true);
    try {
      const res = await apiPost<{ id: string; name: string }>(`${COHORTS_BASE}/cohorts`, {
        name: newCohortName.trim() || 'Study cohort',
        description: 'Created via guided study setup',
        workspace_id: state.workspaceId || null,
        tags: ['study-setup'],
        filter_criteria: {},
      });
      setState((s) => ({ ...s, cohortId: res.id, cohortName: res.name }));
    } finally {
      setCreatingCohort(false);
    }
  }

  return (
    <div className="space-y-6 max-w-3xl" data-testid="study-setup-wizard">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t('study.title')}</h1>
        <p className="text-muted-foreground">{t('study.subtitle')}</p>
        <p className="mt-3 text-sm border rounded-md p-3 bg-muted/30 text-muted-foreground">
          {t('study.pilotDisclaimer')}
        </p>
      </div>

      <ol className="flex flex-wrap gap-2 text-xs">
        {STEPS.map((s) => (
          <li
            key={s.id}
            className={`flex items-center gap-1 rounded-full px-2 py-1 border ${
              s.id === step ? 'border-primary bg-primary/10 text-primary' : s.id < step ? 'border-green-500/40 text-green-700 dark:text-green-400' : 'border-border text-muted-foreground'
            }`}
          >
            {s.id < step ? <CheckCircle2 className="h-3 w-3" /> : <Circle className="h-3 w-3" />}
            {t(s.key)}
          </li>
        ))}
      </ol>

      <Card>
        <CardHeader>
          <CardTitle>{t(STEPS.find((s) => s.id === step)?.key ?? 'study.title')}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {step === 1 && (
            <>
              <p className="text-sm text-muted-foreground">{t('study.step1Body')}</p>
              {authConfig?.require_auth ? (
                passportJwt ? (
                  <p className="text-sm text-green-600 dark:text-green-400">{t('study.signedIn')}</p>
                ) : (
                  <Button
                    className="gap-2"
                    onClick={() => {
                      const url = authConfig.broker_login_url;
                      if (url) window.location.href = buildBrokerLoginUrl(url);
                    }}
                  >
                    <LogIn className="h-4 w-4" />
                    {t('common.signIn')}
                  </Button>
                )
              ) : (
                <p className="text-sm">
                  {t('study.demoUser')}: <code className="rounded bg-muted px-1">demo-user</code>
                </p>
              )}
            </>
          )}

          {step === 2 && (
            <>
              <p className="text-sm text-muted-foreground">{t('study.step2Body')}</p>
              <div className="space-y-2">
                <Label>{t('study.workspaceLabel')}</Label>
                <select
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
                  value={state.workspaceId}
                  onChange={(e) => setState((s) => ({ ...s, workspaceId: e.target.value }))}
                >
                  {wsList.map((w) => (
                    <option key={w.id} value={w.id}>
                      {w.name}
                    </option>
                  ))}
                  {!wsList.length && <option value={state.workspaceId}>{state.workspaceId}</option>}
                </select>
              </div>
              <Button asChild variant="outline">
                <Link to={'/workspaces/new' as any}>{t('study.createWorkspace')}</Link>
              </Button>
            </>
          )}

          {step === 3 && (
            <>
              <p className="text-sm text-muted-foreground">{t('study.step3Body')}</p>
              <div className="flex flex-wrap gap-2">
                <ImportToDrsDialog linkToWorkspaceId={state.workspaceId} />
                <LinkWorkspaceDataDialog workspaceId={state.workspaceId} />
              </div>
              <Button asChild variant="outline">
                <Link to={'/data' as any}>{t('study.openDataBrowser')}</Link>
              </Button>
            </>
          )}

          {step === 4 && (
            <>
              <p className="text-sm text-muted-foreground">{t('study.step4Body')}</p>
              <RegisterToolDialog
                preset={registerPreset}
                onPresetApplied={() => setRegisterPreset(null)}
              />
              <Button asChild variant="outline">
                <Link to={'/tools' as any}>{t('study.openTools')}</Link>
              </Button>
            </>
          )}

          {step === 5 && (
            <>
              <p className="text-sm text-muted-foreground">{t('study.step5Body')}</p>
              <div className="flex gap-2">
                <Input
                  placeholder={t('study.cohortNamePlaceholder')}
                  value={newCohortName}
                  onChange={(e) => setNewCohortName(e.target.value)}
                />
                <Button type="button" disabled={creatingCohort} onClick={() => void createCohort()}>
                  {t('study.createCohort')}
                </Button>
              </div>
              {state.cohortId && (
                <p className="text-sm">
                  {t('study.activeCohort')}:{' '}
                  <Link to={`/cohorts/${state.cohortId}` as any} className="text-primary hover:underline">
                    {state.cohortName}
                  </Link>
                </p>
              )}
              {state.cohortId && <SampleSheetImportDialog cohortId={state.cohortId} />}
            </>
          )}

          {step === 6 && state.cohortId && (
            <>
              <p className="text-sm text-muted-foreground">{t('study.step6Body')}</p>
              <p className="text-xs text-muted-foreground border rounded-md p-3 bg-muted/30">
                {t('analysisWizard.inputCohortHint')}
              </p>
              <RunCohortDialog
                cohortId={state.cohortId}
                cohortName={state.cohortName}
                workspaceId={state.workspaceId}
                open={runDialogOpen}
                onOpenChange={setRunDialogOpen}
                hideTrigger
              />
              <Button className="gap-2" onClick={() => setRunDialogOpen(true)}>
                <Play className="h-4 w-4" />
                {t('cohort.runOnCohort')}
              </Button>
            </>
          )}

          {step === 7 && (
            <>
              <p className="text-sm text-muted-foreground">{t('study.step7Body')}</p>
              <ul className="space-y-2 text-sm">
                {(runs?.runs ?? []).map((r) => (
                  <li key={r.run_id} className="flex justify-between border-b pb-2">
                    <Link to={`/workflows/runs/${r.run_id}` as any} className="font-mono hover:underline">
                      {r.run_id.slice(0, 14)}…
                    </Link>
                    <span className="text-muted-foreground">{r.state ?? '—'}</span>
                  </li>
                ))}
                {!(runs?.runs ?? []).length && (
                  <li className="text-muted-foreground">{t('study.noRunsYet')}</li>
                )}
              </ul>
              <Button asChild variant="outline">
                <Link to={'/workflows' as any}>{t('study.openWorkflows')}</Link>
              </Button>
            </>
          )}
        </CardContent>
      </Card>

      <div className="flex justify-between">
        <Button
          variant="outline"
          className="gap-2"
          disabled={step <= 1}
          onClick={() => setState((s) => ({ ...s, step: Math.max(1, s.step - 1) }))}
        >
          <ArrowLeft className="h-4 w-4" />
          {t('study.back')}
        </Button>
        <Button
          className="gap-2"
          disabled={step >= 7 || (step === 5 && !state.cohortId)}
          onClick={() => setState((s) => ({ ...s, step: Math.min(7, s.step + 1) }))}
        >
          {t('study.next')}
          <ArrowRight className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
