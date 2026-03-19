import { Badge } from '@/components/ui/badge';
import type { WesState } from '@/api/types';

const stateVariant: Record<WesState, 'default' | 'secondary' | 'destructive' | 'success' | 'warning' | 'outline'> = {
  UNKNOWN: 'outline',
  QUEUED: 'secondary',
  INITIALIZING: 'secondary',
  RUNNING: 'default',
  PAUSED: 'warning',
  COMPLETE: 'success',
  EXECUTOR_ERROR: 'destructive',
  SYSTEM_ERROR: 'destructive',
  CANCELED: 'outline',
  CANCELING: 'warning',
};

export function WorkflowStateBadge({ state }: { state: WesState }) {
  return <Badge variant={stateVariant[state] ?? 'outline'}>{state}</Badge>;
}
