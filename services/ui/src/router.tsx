import { lazy, Suspense, type ComponentType } from 'react';
import { createRootRoute, createRoute, Outlet } from '@tanstack/react-router';
import { AppLayout } from '@/components/AppLayout';
import { PageLoader } from '@/components/PageLoader';
import { AuthCallback } from '@/pages/AuthCallback';

function lazyPage<T extends Record<string, ComponentType<unknown>>>(
  importer: () => Promise<T>,
  exportName: keyof T,
) {
  const Lazy = lazy(() =>
    importer().then((module) => ({ default: module[exportName] as ComponentType })),
  );
  return function LazyRoutePage() {
    return (
      <Suspense fallback={<PageLoader />}>
        <Lazy />
      </Suspense>
    );
  };
}

const Dashboard = lazyPage(() => import('@/pages/Dashboard'), 'Dashboard');
const DataBrowser = lazyPage(() => import('@/pages/DataBrowser'), 'DataBrowser');
const ObjectDetailPage = lazyPage(() => import('@/pages/ObjectDetailPage'), 'ObjectDetailPage');
const WorkflowCenter = lazyPage(() => import('@/pages/WorkflowCenter'), 'WorkflowCenter');
const RunDetailPage = lazyPage(() => import('@/pages/RunDetailPage'), 'RunDetailPage');
const ToolRegistry = lazyPage(() => import('@/pages/ToolRegistry'), 'ToolRegistry');
const BeaconExplorer = lazyPage(() => import('@/pages/BeaconExplorer'), 'BeaconExplorer');
const AccessManagement = lazyPage(() => import('@/pages/AccessManagement'), 'AccessManagement');
const Settings = lazyPage(() => import('@/pages/Settings'), 'Settings');
const CohortListPage = lazyPage(() => import('@/pages/CohortListPage'), 'CohortListPage');
const CohortDetailPage = lazyPage(() => import('@/pages/CohortDetailPage'), 'CohortDetailPage');
const NewCohortPage = lazyPage(() => import('@/pages/NewCohortPage'), 'NewCohortPage');
const WorkspaceListPage = lazyPage(() => import('@/pages/WorkspaceListPage'), 'WorkspaceListPage');
const WorkspaceDetailPage = lazyPage(
  () => import('@/pages/WorkspaceDetailPage'),
  'WorkspaceDetailPage',
);
const InsightsPage = lazyPage(() => import('@/pages/InsightsPage'), 'InsightsPage');
const StudySetupPage = lazyPage(() => import('@/pages/StudySetupPage'), 'StudySetupPage');
const NewWorkspacePage = lazyPage(() => import('@/pages/NewWorkspacePage'), 'NewWorkspacePage');
const MetadataSubmissionsPage = lazyPage(
  () => import('@/pages/MetadataSubmissionsPage'),
  'MetadataSubmissionsPage',
);
const MetadataSubmissionDetailPage = lazyPage(
  () => import('@/pages/MetadataSubmissionDetailPage'),
  'MetadataSubmissionDetailPage',
);

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
  validateSearch: (search: Record<string, unknown>) => ({
    analyze:
      search.analyze === '1' ||
      search.analyze === true ||
      search.analyze === 'true',
  }),
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
const metadataRoute = createRoute({
  getParentRoute: () => layoutRoute,
  path: '/metadata',
  component: MetadataSubmissionsPage,
});
const metadataDetailRoute = createRoute({
  getParentRoute: () => layoutRoute,
  path: '/metadata/submissions/$alias',
  component: MetadataSubmissionDetailPage,
});

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
  metadataRoute,
  metadataDetailRoute,
]);

rootRoute.addChildren([authCallbackRoute, layoutRoute]);

export const routeTree = rootRoute;
