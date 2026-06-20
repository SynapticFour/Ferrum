import { ProblemReportPanel, type ProblemReportPanelProps } from '@/components/ProblemReportPanel';
import { cn } from '@/lib/utils';

export interface ErrorWithReportProps extends ProblemReportPanelProps {
  /** When false, only the report panel is shown (message already rendered elsewhere). */
  showMessage?: boolean;
  messageClassName?: string;
}

export function ErrorWithReport({
  errorMessage,
  showMessage = true,
  messageClassName,
  className,
  ...reportProps
}: ErrorWithReportProps) {
  if (!errorMessage.trim()) return null;

  return (
    <div className={cn('space-y-0', className)}>
      {showMessage && (
        <p className={cn('text-sm text-destructive', messageClassName)}>{errorMessage}</p>
      )}
      <ProblemReportPanel errorMessage={errorMessage} {...reportProps} />
    </div>
  );
}
