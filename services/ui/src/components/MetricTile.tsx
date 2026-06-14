import { Link } from '@tanstack/react-router';
import { Card, CardContent } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import type { LucideIcon } from 'lucide-react';

export function MetricTile({
  to,
  label,
  value,
  icon: Icon,
  iconClassName,
  subtitle,
}: {
  to?: string;
  label: string;
  value: string | number;
  icon: LucideIcon;
  iconClassName?: string;
  subtitle?: string;
}) {
  const inner = (
    <Card
      className={cn(
        'border-border/80 transition-colors',
        to && 'cursor-pointer hover:border-primary/40 hover:bg-muted/30',
      )}
    >
      <CardContent className="pt-6">
        <div className="flex items-center gap-3">
          <div className={cn('rounded-lg p-2', iconClassName ?? 'bg-primary/10')}>
            <Icon className={cn('h-5 w-5', iconClassName ? '' : 'text-primary')} />
          </div>
          <div>
            <p className="text-xs font-medium text-muted-foreground">{label}</p>
            <p className="text-2xl font-bold tabular-nums">{value}</p>
            {subtitle && <p className="text-xs text-muted-foreground mt-0.5">{subtitle}</p>}
          </div>
        </div>
      </CardContent>
    </Card>
  );

  if (!to) return inner;
  return (
    <Link to={to as '/'} className="block no-underline text-inherit">
      {inner}
    </Link>
  );
}
