import { Link, useRouterState } from '@tanstack/react-router';
import { useThemeStore } from '@/stores/theme';
import { useAuthStore } from '@/stores/auth';
import { useAuthConfig } from '@/hooks/useAuthConfig';
import { buildBrokerLoginUrl } from '@/lib/auth';
import { LayoutDashboard, Database, Workflow, Wrench, Dna, Shield, Settings, Moon, Sun, Users, FolderOpen, LogIn, LogOut, BarChart3, Compass } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Footer } from '@/components/Footer';
import { LanguageSwitcher } from '@/components/LanguageSwitcher';
import { useI18n } from '@/i18n/I18nProvider';

const navItems = [
  { path: '/', labelKey: 'nav.dashboard', icon: LayoutDashboard },
  { path: '/study/setup', labelKey: 'nav.studySetup', icon: Compass },
  { path: '/workspaces', labelKey: 'nav.workspaces', icon: FolderOpen },
  { path: '/data', labelKey: 'nav.data', icon: Database },
  { path: '/cohorts', labelKey: 'nav.cohorts', icon: Users },
  { path: '/workflows', labelKey: 'nav.workflows', icon: Workflow },
  { path: '/tools', labelKey: 'nav.tools', icon: Wrench },
  { path: '/beacon', labelKey: 'nav.beacon', icon: Dna },
  { path: '/insights', labelKey: 'nav.insights', icon: BarChart3 },
  { path: '/access', labelKey: 'nav.access', icon: Shield },
  { path: '/settings', labelKey: 'nav.settings', icon: Settings },
] as const;

export function AppLayout({ children }: { children: React.ReactNode }) {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const dark = useThemeStore((s) => s.dark);
  const toggleDark = useThemeStore((s) => s.toggle);
  const passportJwt = useAuthStore((s) => s.passportJwt);
  const setPassport = useAuthStore((s) => s.setPassport);
  const { data: authConfig } = useAuthConfig();
  const { t, dir } = useI18n();

  const brokerLoginUrl = authConfig?.broker_login_url;
  const showSignIn = Boolean(authConfig?.require_auth && brokerLoginUrl && !passportJwt);

  const handleSignIn = () => {
    if (!brokerLoginUrl) return;
    window.location.href = buildBrokerLoginUrl(brokerLoginUrl);
  };

  const handleSignOut = () => setPassport(null);

  return (
    <div className={cn('min-h-screen bg-background flex flex-col', dark && 'dark')} dir={dir}>
      <aside
        className={cn(
          'fixed top-0 z-40 h-screen w-56 border-border bg-card',
          dir === 'rtl' ? 'right-0 border-l' : 'left-0 border-r',
        )}
      >
        <div className="flex h-14 items-center justify-between gap-2 border-b border-border px-4">
          <span className="font-semibold text-primary">Ferrum</span>
          {showSignIn ? (
            <Button variant="ghost" size="icon" onClick={handleSignIn} title={t('common.signIn')}>
              <LogIn className="h-4 w-4" />
            </Button>
          ) : passportJwt ? (
            <Button variant="ghost" size="icon" onClick={handleSignOut} title={t('common.signOut')}>
              <LogOut className="h-4 w-4" />
            </Button>
          ) : null}
        </div>
        <nav className="space-y-0.5 p-2">
          {navItems.map(({ path, labelKey, icon: Icon }) => (
            <Link
              key={path}
              to={path as '/'}
              className={cn(
                'flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors',
                pathname === path || (path !== '/' && pathname.startsWith(path))
                  ? 'bg-primary/10 text-primary'
                  : 'text-muted-foreground hover:bg-muted hover:text-foreground',
              )}
            >
              <Icon className="h-4 w-4 shrink-0" />
              {t(labelKey)}
            </Link>
          ))}
        </nav>
        <div
          className={cn(
            'absolute bottom-4 flex flex-col gap-2',
            dir === 'rtl' ? 'right-4 left-4' : 'left-4 right-4',
          )}
        >
          <LanguageSwitcher />
          <Button variant="ghost" size="icon" onClick={toggleDark} className="self-start">
            {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </Button>
        </div>
      </aside>
      <main className={cn('flex-1 flex flex-col', dir === 'rtl' ? 'pr-56' : 'pl-56')}>
        <div className="container max-w-7xl py-6 flex-1">{children}</div>
        <Footer />
      </main>
    </div>
  );
}
