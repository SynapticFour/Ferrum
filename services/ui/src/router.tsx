import { createRootRoute, createRoute, Outlet } from '@tanstack/react-router';
import { AppLayout } from '@/components/AppLayout';
import { Dashboard } from '@/pages/Dashboard';
import { DataBrowser } from '@/pages/DataBrowser';
import { WorkflowCenter } from '@/pages/WorkflowCenter';
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
const workflowsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/workflows', component: WorkflowCenter });
const toolsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/tools', component: ToolRegistry });
const beaconRoute = createRoute({ getParentRoute: () => rootRoute, path: '/beacon', component: BeaconExplorer });
const accessRoute = createRoute({ getParentRoute: () => rootRoute, path: '/access', component: AccessManagement });
const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: Settings });

rootRoute.addChildren([
  indexRoute,
  dataRoute,
  workflowsRoute,
  toolsRoute,
  beaconRoute,
  accessRoute,
  settingsRoute,
]);

export const routeTree = rootRoute;
