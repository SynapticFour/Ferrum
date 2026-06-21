import { useEffect, useState, type ReactNode } from 'react';
import { Link } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiGet, apiPost } from '@/api/client';
import { useAuthConfig } from '@/hooks/useAuthConfig';
import { useAuthStore } from '@/stores/auth';
import { buildBrokerLoginUrl, isPassportExpired } from '@/lib/auth';
import { isFlyPilot } from '@/lib/pilotContext';
import { useAdminConfig } from '@/hooks/useAdminConfig';
import { useI18n } from '@/i18n/I18nProvider';
import { ImportToDrsDialog } from '@/components/ImportToDrsDialog';
import { LinkWorkspaceDataDialog } from '@/components/LinkWorkspaceDataDialog';
import { RegisterToolDialog } from '@/components/RegisterToolDialog';
import { SampleSheetImportDialog } from '@/components/SampleSheetImportDialog';
import { RunCohortDialog } from '@/components/RunCohortDialog';
import { NoopExecutorBanner } from '@/components/NoopExecutorBanner';
import { type RegisterToolPreset } from '@/lib/trsCatalogs';
import {
  CheckCircle2,
  LogIn,
  Play,
} from 'lucide-react';

const STORAGE_KEY = 'ferrum-study-setup-v1';
const COHORTS_BASE = '/cohorts/v1';

interface SetupState {
  workspaceId: string;
  cohortId: string;
  cohortName: string;
}

const DEFAULT_STATE: SetupState = {
  workspaceId: '',
  cohortId: 'demo-cohort-01',
  cohortName: 'Demo cohort',
};

function loadState(): SetupState {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_STATE, ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return DEFAULT_STATE;
}

function saveState(s: SetupState) {
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

function TopicCard({
  title,
  isTitle,
  notTitle,
  body,
  children,
}: {
  title: string;
  isTitle: string;
  notTitle: string;
  body: string;
  children?: ReactNode;
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-lg">{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3 text-sm">
        <p className="text-muted-foreground">{body}</p>
        <div className="grid gap-2 sm:grid-cols-2 text-xs">
          <p className="rounded-md border border-green-500/30 bg-green-500/5 p-2 text-green-900 dark:text-green-200">
            <span className="font-medium">{isTitle}</span>
          </p>
          <p className="rounded-md border border-border bg-muted/30 p-2 text-muted-foreground">
            <span className="font-medium text-foreground">{notTitle}</span>
          </p>
        </div>
        {children}
      </CardContent>
    </Card>
  );
}

export function StudySetupPage() {
  const { t } = useI18n();
  const { data: authConfig } = useAuthConfig();
  const { data: adminConfig } = useAdminConfig();
  const passportJwt = useAuthStore((s) => s.passportJwt);
  const signedIn = Boolean(passportJwt && !isPassportExpired(passportJwt));
  const requireAuth = Boolean(authConfig?.require_auth);
  const flyPilot = isFlyPilot(adminConfig);

  const [state, setState] = useState<SetupState>(loadState);
  const [newCohortName, setNewCohortName] = useState('');
  const [creatingCohort, setCreatingCohort] = useState(false);
  const [runDialogOpen, setRunDialogOpen] = useState(false);
  const [registerPreset, setRegisterPreset] = useState<RegisterToolPreset | null>(null);

  useEffect(() => {
    saveState(state);
  }, [state]);

  const { data: workspaces } = useQuery({
    queryKey: ['workspaces', 'mine'],
    queryFn: () => apiGet<Array<{ id: string; name: string }>>('/workspaces/v1/workspaces'),
    enabled: signedIn,
    retry: false,
  });

  const { data: runs } = useQuery({
    queryKey: ['wes', 'runs', 'study', state.workspaceId],
    queryFn: () =>
      apiGet<{ runs: Array<{ run_id: string; state?: string }> }>(
        `/ga4gh/wes/v1/runs?workspace_id=${encodeURIComponent(state.workspaceId)}&page_size=10`,
      ),
    enabled: signedIn && Boolean(state.workspaceId),
  });

  const wsList = Array.isArray(workspaces) ? workspaces : [];

  useEffect(() => {
    if (wsList.length === 0) return;
    const ids = new Set(wsList.map((w) => w.id));
    if (!state.workspaceId || !ids.has(state.workspaceId)) {
      setState((s) => ({ ...s, workspaceId: wsList[0].id }));
    }
  }, [wsList, state.workspaceId]);

  async function createCohort() {
    if (!state.workspaceId) return;
    setCreatingCohort(true);
    try {
      const res = await apiPost<{ id: string; name: string }>(`${COHORTS_BASE}/cohorts`, {
        name: newCohortName.trim() || 'Study cohort',
        description: 'Created via pilot orientation',
        workspace_id: state.workspaceId,
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
        <p className="mt-2 text-sm text-muted-foreground">{t('study.notWorkflow')}</p>
        <div className="mt-3">
          <NoopExecutorBanner compact />
        </div>
      </div>

      {requireAuth && !signedIn && (
        <TopicCard
          title={t('study.topicIdentityTitle')}
          isTitle={t('study.topicIdentityIs')}
          notTitle={t('study.topicIdentityNot')}
          body={t('study.step1Body')}
        >
          {authConfig?.broker_login_url && (
            <Button
              className="gap-2"
              onClick={() => {
                window.location.href = buildBrokerLoginUrl(authConfig.broker_login_url!);
              }}
            >
              <LogIn className="h-4 w-4" />
              {t('common.signIn')}
            </Button>
          )}
        </TopicCard>
      )}

      {requireAuth && signedIn && (
        <p className="flex items-center gap-2 text-sm text-green-600 dark:text-green-400">
          <CheckCircle2 className="h-4 w-4 shrink-0" />
          {t('study.signedIn')}
        </p>
      )}

      {!requireAuth && (
        <TopicCard
          title={t('study.topicIdentityTitle')}
          isTitle={t('study.topicIdentityIsLocal')}
          notTitle={t('study.topicIdentityNot')}
          body={t('study.step1Body')}
        >
          <p>
            {t('study.demoUser')}: <code className="rounded bg-muted px-1">demo-user</code>
          </p>
        </TopicCard>
      )}

      <TopicCard
        title={t('study.topicWorkspaceTitle')}
        isTitle={t('study.topicWorkspaceIs')}
        notTitle={t('study.topicWorkspaceNot')}
        body={t('study.step2Body')}
      >
        {!signedIn && requireAuth ? (
          <p className="text-xs text-muted-foreground">{t('study.signInForWorkspace')}</p>
        ) : wsList.length === 0 ? (
          <div className="space-y-2">
            <p className="text-xs text-muted-foreground">
              {flyPilot ? t('study.noWorkspacePilot') : t('study.noWorkspaceYet')}
            </p>
            <Button asChild variant="outline" size="sm">
              <Link to={'/workspaces/new' as any}>{t('study.createWorkspace')}</Link>
            </Button>
          </div>
        ) : (
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
            </select>
            <Button asChild variant="outline" size="sm">
              <Link to={'/workspaces/new' as any}>{t('study.createWorkspace')}</Link>
            </Button>
          </div>
        )}
      </TopicCard>

      <TopicCard
        title={t('study.topicDataTitle')}
        isTitle={t('study.topicDataIs')}
        notTitle={t('study.topicDataNot')}
        body={t('study.step3Body')}
      >
        <div className="flex flex-wrap gap-2">
          {state.workspaceId ? (
            <>
              <ImportToDrsDialog linkToWorkspaceId={state.workspaceId} />
              <LinkWorkspaceDataDialog workspaceId={state.workspaceId} />
            </>
          ) : (
            <ImportToDrsDialog />
          )}
          <Button asChild variant="outline" size="sm">
            <Link to={'/data' as any}>{t('study.openDataBrowser')}</Link>
          </Button>
        </div>
      </TopicCard>

      <TopicCard
        title={t('study.topicToolsTitle')}
        isTitle={t('study.topicToolsIs')}
        notTitle={t('study.topicToolsNot')}
        body={t('study.step4Body')}
      >
        <RegisterToolDialog preset={registerPreset} onPresetApplied={() => setRegisterPreset(null)} />
        <Button asChild variant="outline" size="sm">
          <Link to={'/tools' as any}>{t('study.openTools')}</Link>
        </Button>
      </TopicCard>

      <TopicCard
        title={t('study.topicCohortTitle')}
        isTitle={t('study.topicCohortIs')}
        notTitle={t('study.topicCohortNot')}
        body={t('study.step5Body')}
      >
        <div className="flex gap-2">
          <Input
            placeholder={t('study.cohortNamePlaceholder')}
            value={newCohortName}
            onChange={(e) => setNewCohortName(e.target.value)}
            disabled={!state.workspaceId}
          />
          <Button
            type="button"
            disabled={creatingCohort || !state.workspaceId}
            onClick={() => void createCohort()}
          >
            {t('study.createCohort')}
          </Button>
        </div>
        {state.cohortId && (
          <p>
            {t('study.activeCohort')}:{' '}
            <Link to={`/cohorts/${state.cohortId}` as any} className="text-primary hover:underline">
              {state.cohortName}
            </Link>
          </p>
        )}
        {state.cohortId && <SampleSheetImportDialog cohortId={state.cohortId} />}
      </TopicCard>

      <TopicCard
        title={t('study.topicRunTitle')}
        isTitle={t('study.topicRunIs')}
        notTitle={t('study.topicRunNot')}
        body={t('study.step6Body')}
      >
        {state.cohortId && state.workspaceId ? (
          <>
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
            <Button className="gap-2" size="sm" onClick={() => setRunDialogOpen(true)}>
              <Play className="h-4 w-4" />
              {t('cohort.runOnCohort')}
            </Button>
          </>
        ) : (
          <p className="text-xs text-muted-foreground">{t('study.runNeedsWorkspaceCohort')}</p>
        )}
      </TopicCard>

      <TopicCard
        title={t('study.topicResultsTitle')}
        isTitle={t('study.topicResultsIs')}
        notTitle={t('study.topicResultsNot')}
        body={t('study.step7Body')}
      >
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
        <Button asChild variant="outline" size="sm">
          <Link to={'/workflows' as any}>{t('study.openWorkflows')}</Link>
        </Button>
      </TopicCard>
    </div>
  );
}
