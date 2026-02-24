import { cn } from '@/lib/utils';

type Status = 'up' | 'degraded' | 'down';

const styles: Record<Status, string> = {
  up: 'bg-emerald-600/20 text-emerald-400 border-emerald-500/30',
  degraded: 'bg-amber-600/20 text-amber-400 border-amber-500/30',
  down: 'bg-red-600/20 text-red-400 border-red-500/30',
};

export function ServiceHealthBadge({
  status,
  label,
  className,
}: {
  status: Status;
  label: string;
  className?: string;
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium',
        styles[status],
        className
      )}
    >
      <span className={cn('h-1.5 w-1.5 rounded-full', status === 'up' && 'bg-emerald-400', status === 'degraded' && 'bg-amber-400', status === 'down' && 'bg-red-400')} />
      {label}
    </span>
  );
}
