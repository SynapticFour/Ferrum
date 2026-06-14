import { useEffect } from 'react';
import { useNavigate } from '@tanstack/react-router';
import { parseTokenFromLocationHash, storePassport } from '@/lib/auth';
import { useAuthStore } from '@/stores/auth';

export function AuthCallback() {
  const navigate = useNavigate();
  const setPassport = useAuthStore((s) => s.setPassport);

  useEffect(() => {
    const token = parseTokenFromLocationHash(window.location.hash);
    if (token) {
      storePassport(token);
      setPassport(token);
    }
    void navigate({ to: '/', replace: true });
  }, [navigate, setPassport]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6">
      <p className="text-muted-foreground text-sm">Completing sign-in…</p>
    </div>
  );
}
