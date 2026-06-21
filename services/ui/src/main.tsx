import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, createRouter } from '@tanstack/react-router';
import { routeTree } from './router';
import { I18nProvider } from './i18n/I18nProvider';
import './index.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 30_000, retry: 1 },
  },
});

function routerBasepath(): string | undefined {
  const base = (import.meta.env.BASE_URL ?? '/').replace(/\/$/, '');
  return base.length > 0 ? base : undefined;
}

const router = createRouter({
  routeTree,
  basepath: routerBasepath(),
});

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>
    </QueryClientProvider>
  </React.StrictMode>
);
