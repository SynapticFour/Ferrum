import { createRootRoute, createRoute, Outlet } from '@tanstack/react-router';
import { AppLayout } from '@/components/AppLayout';
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
import { NewWorkspacePage } from '@/pages/NewWorkspacePage';

const rootRoute = createRootRoute({
  component: () => (
    <AppLayout>
      <Outlet />
    </AppLayout>
  ),
});

const indexRoute = createRoute({ getParentRoute: () => rootRoute, path: '/', component: Dashboard });
const dataRoute = createRoute({ getParentRoute: () => rootRoute, path: '/data', component: DataBrowser });
const objectDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/data/objects/$objectId',
  component: ObjectDetailPage,
});
const workflowsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/workflows', component: WorkflowCenter });
const runDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/workflows/runs/$runId',
  component: RunDetailPage,
});
const toolsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/tools', component: ToolRegistry });
const beaconRoute = createRoute({ getParentRoute: () => rootRoute, path: '/beacon', component: BeaconExplorer });
const accessRoute = createRoute({ getParentRoute: () => rootRoute, path: '/access', component: AccessManagement });
const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: Settings });
const cohortsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/cohorts', component: CohortListPage });
const cohortNewRoute = createRoute({ getParentRoute: () => rootRoute, path: '/cohorts/new', component: NewCohortPage });
const cohortDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/cohorts/$cohortId',
  component: CohortDetailPage,
});
const workspacesRoute = createRoute({ getParentRoute: () => rootRoute, path: '/workspaces', component: WorkspaceListPage });
const workspaceDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/workspaces/$workspaceId',
  component: WorkspaceDetailPage,
});
const newWorkspaceRoute = createRoute({ getParentRoute: () => rootRoute, path: '/workspaces/new', component: NewWorkspacePage });

rootRoute.addChildren([
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
  newWorkspaceRoute,
]);

export const routeTree = rootRoute;
