import { Link } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { dismissTesterWelcomeGuide } from '@/components/TesterWelcomeDialog';
import { parseTokenFromLocationHash, storePassport } from '@/lib/auth';
import { useAuthStore } from '@/stores/auth';
import { useI18n } from '@/i18n/I18nProvider';

export function AuthCallback() {
  const navigate = useNavigate();
  const setPassport = useAuthStore((s) => s.setPassport);
  const { t } = useI18n();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const token = parseTokenFromLocationHash(window.location.hash);
    if (token) {
      storePassport(token);
      setPassport(token);
      dismissTesterWelcomeGuide();
      window.history.replaceState(null, '', window.location.pathname + window.location.search);
      void navigate({ to: '/', replace: true });
    } else {
      setFailed(true);
    }
  }, [navigate, setPassport]);

  if (failed) {
    return (
      <div className="flex min-h-screen flex-col items-center justify-center gap-4 bg-background p-6 text-center">
        <div>
          <p className="font-medium">{t('auth.signInFailed')}</p>
          <p className="mt-2 text-sm text-muted-foreground max-w-md">{t('auth.signInFailedHint')}</p>
        </div>
        <Button asChild variant="default">
          <Link to={'/settings' as any}>{t('auth.tryAgain')}</Link>
        </Button>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6">
      <p className="text-muted-foreground text-sm">{t('auth.completingSignIn')}</p>
    </div>
  );
}
