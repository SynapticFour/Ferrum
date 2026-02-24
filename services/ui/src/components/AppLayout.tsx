import { Link, useRouterState } from '@tanstack/react-router';
import { useThemeStore } from '@/stores/theme';
import { LayoutDashboard, Database, Workflow, Wrench, Dna, Shield, Settings, Moon, Sun, Users } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';

const nav = [
  { path: '/', label: 'Dashboard', icon: LayoutDashboard },
  { path: '/data', label: 'Data Browser', icon: Database },
  { path: '/cohorts', label: 'Cohorts', icon: Users },
  { path: '/workflows', label: 'Workflows', icon: Workflow },
  { path: '/tools', label: 'Tool Registry', icon: Wrench },
  { path: '/beacon', label: 'Beacon', icon: Dna },
  { path: '/access', label: 'Access', icon: Shield },
  { path: '/settings', label: 'Settings', icon: Settings },
];

export function AppLayout({ children }: { children: React.ReactNode }) {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const dark = useThemeStore((s) => s.dark);
  const toggleDark = useThemeStore((s) => s.toggle);

  return (
    <div className={cn('min-h-screen bg-background', dark && 'dark')}>
      <aside className="fixed left-0 top-0 z-40 h-screen w-56 border-r border-border bg-card">
        <div className="flex h-14 items-center gap-2 border-b border-border px-4">
          <span className="font-semibold text-primary">Ferrum</span>
        </div>
        <nav className="space-y-0.5 p-2">
          {nav.map(({ path, label, icon: Icon }) => (
            <Link
              key={path}
              to={path}
              className={cn(
                'flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors',
                pathname === path || (path !== '/' && pathname.startsWith(path))
                  ? 'bg-primary/10 text-primary'
                  : 'text-muted-foreground hover:bg-muted hover:text-foreground'
              )}
            >
              <Icon className="h-4 w-4 shrink-0" />
              {label}
            </Link>
          ))}
        </nav>
        <div className="absolute bottom-4 left-4">
          <Button variant="ghost" size="icon" onClick={toggleDark}>
            {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </Button>
        </div>
      </aside>
      <main className="pl-56">
        <div className="container max-w-7xl py-6">{children}</div>
      </main>
    </div>
  );
}
