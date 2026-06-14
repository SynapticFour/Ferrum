import { createRootRoute, createRoute, Outlet } from '@tanstack/react-router';
import { AppLayout } from '@/components/AppLayout';
import { AuthCallback } from '@/pages/AuthCallback';
import { Dashboard } from '@/pages/Dashboard';
import { DataBrowser } from '@/pages/DataBrowser';
import { ObjectDetailPage } from '@/pages/ObjectDetailPage';
import { WorkflowCenter } from '@/pages/WorkflowCenter';
import { RunDetailPage } from '@/pages/RunDetailPage';
import { ToolRegistry } from '@/pages/ToolRegistry';
import { BeaconExplorer } from '@/pages/BeaconExplorer';
import { AccessManagement } from '@/pages/AccessManagement';
import { Settings } from '@/pages/Settings';
import { CohortListPage } from '@/pages/CohortListPage';
import { CohortDetailPage } from '@/pages/CohortDetailPage';
import { NewCohortPage } from '@/pages/NewCohortPage';
import { WorkspaceListPage } from '@/pages/WorkspaceListPage';
import { WorkspaceDetailPage } from '@/pages/WorkspaceDetailPage';
import { InsightsPage } from '@/pages/InsightsPage';
import { StudySetupPage } from '@/pages/StudySetupPage';
import { NewWorkspacePage } from '@/pages/NewWorkspacePage';

const rootRoute = createRootRoute({
  component: () => <Outlet />,
});

const authCallbackRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/auth/callback',
  component: AuthCallback,
});

const layoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: 'layout',
  component: () => (
    <AppLayout>
      <Outlet />
    </AppLayout>
  ),
});

const indexRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/', component: Dashboard });
const dataRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/data', component: DataBrowser });
const objectDetailRoute = createRoute({
  getParentRoute: () => layoutRoute,
  path: '/data/objects/$objectId',
  component: ObjectDetailPage,
});
const workflowsRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/workflows', component: WorkflowCenter });
const runDetailRoute = createRoute({
  getParentRoute: () => layoutRoute,
  path: '/workflows/runs/$runId',
  component: RunDetailPage,
});
const toolsRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/tools', component: ToolRegistry });
const beaconRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/beacon', component: BeaconExplorer });
const accessRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/access', component: AccessManagement });
const settingsRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/settings', component: Settings });
const cohortsRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/cohorts', component: CohortListPage });
const cohortNewRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/cohorts/new', component: NewCohortPage });
const cohortDetailRoute = createRoute({
  getParentRoute: () => layoutRoute,
  path: '/cohorts/$cohortId',
  component: CohortDetailPage,
});
const workspacesRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/workspaces', component: WorkspaceListPage });
const workspaceDetailRoute = createRoute({
  getParentRoute: () => layoutRoute,
  path: '/workspaces/$workspaceId',
  component: WorkspaceDetailPage,
});
const insightsRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/insights', component: InsightsPage });
const studySetupRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/study/setup', component: StudySetupPage });
const newWorkspaceRoute = createRoute({ getParentRoute: () => layoutRoute, path: '/workspaces/new', component: NewWorkspacePage });

layoutRoute.addChildren([
  indexRoute,
  dataRoute,
  objectDetailRoute,
  workflowsRoute,
  runDetailRoute,
  toolsRoute,
  beaconRoute,
  accessRoute,
  settingsRoute,
  cohortsRoute,
  cohortNewRoute,
  cohortDetailRoute,
  workspacesRoute,
  workspaceDetailRoute,
  insightsRoute,
  studySetupRoute,
  newWorkspaceRoute,
]);

rootRoute.addChildren([authCallbackRoute, layoutRoute]);

export const routeTree = rootRoute;
