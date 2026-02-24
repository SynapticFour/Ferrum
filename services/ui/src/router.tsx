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
]);

export const routeTree = rootRoute;
