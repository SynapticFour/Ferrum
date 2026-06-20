import { useEffect, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { BookOpen, Database, LogIn, X } from 'lucide-react';
import { useAuthConfig } from '@/hooks/useAuthConfig';
import { useAdminConfig, isNoopExecutor } from '@/hooks/useAdminConfig';
import { buildBrokerLoginUrl } from '@/lib/auth';
import { isFlyPilot } from '@/lib/pilotContext';
import { useI18n } from '@/i18n/I18nProvider';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

const STORAGE_KEY = 'ferrum.tester-guide-v1';

export function TesterWelcomeDialog() {
  const { t } = useI18n();
  const { data: authConfig } = useAuthConfig();
  const { data: adminConfig } = useAdminConfig();
  const [open, setOpen] = useState(false);

  const brokerLoginUrl = authConfig?.broker_login_url;
  const showForPilot = isFlyPilot(adminConfig);

  useEffect(() => {
    if (!showForPilot) return;
    try {
      if (localStorage.getItem(STORAGE_KEY) === 'dismissed') return;
    } catch {
      /* private browsing */
    }
    setOpen(true);
  }, [showForPilot]);

  const dismiss = () => {
    try {
      localStorage.setItem(STORAGE_KEY, 'dismissed');
    } catch {
      /* ignore */
    }
    setOpen(false);
  };

  if (!showForPilot) return null;

  const bullets = [
    t('testerGuide.bulletUi'),
    t('testerGuide.bulletSignIn'),
    t('testerGuide.bulletColdStart'),
    t('testerGuide.bulletImport'),
    isNoopExecutor(adminConfig) ? t('testerGuide.bulletNoop') : null,
    t('testerGuide.bulletReport'),
  ].filter(Boolean) as string[];

  return (
    <Dialog open={open} onOpenChange={(next) => (next ? setOpen(true) : dismiss())}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <BookOpen className="h-5 w-5 text-primary" />
            {t('testerGuide.title')}
          </DialogTitle>
          <DialogDescription>{t('testerGuide.intro')}</DialogDescription>
        </DialogHeader>
        <ul className="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
          {bullets.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
        <p className="text-xs text-muted-foreground">{t('testerGuide.pauseNote')}</p>
        <DialogFooter className="flex-col gap-2 sm:flex-row sm:justify-between">
          <div className="flex flex-wrap gap-2">
            {brokerLoginUrl && (
              <Button
                size="sm"
                className="gap-2"
                onClick={() => {
                  window.location.href = buildBrokerLoginUrl(brokerLoginUrl);
                }}
              >
                <LogIn className="h-4 w-4" />
                {t('common.signIn')}
              </Button>
            )}
            <Button asChild size="sm" variant="outline" className="gap-2">
              <Link to={'/data' as any}>
                <Database className="h-4 w-4" />
                {t('testerGuide.browseData')}
              </Link>
            </Button>
          </div>
          <Button size="sm" variant="ghost" className="gap-2" onClick={dismiss}>
            <X className="h-4 w-4" />
            {t('testerGuide.dismiss')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
