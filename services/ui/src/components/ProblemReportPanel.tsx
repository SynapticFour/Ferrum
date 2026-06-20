import { useState } from 'react';
import { Bug, Copy, Github } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useAdminConfig } from '@/hooks/useAdminConfig';
import { useI18n } from '@/i18n/I18nProvider';
import {
  buildGitHubIssueUrl,
  buildMailtoUrl,
  buildProblemReportBody,
  isProblemReportEnabled,
  type ProblemReportInput,
} from '@/lib/reportProblem';

export interface ProblemReportPanelProps extends Omit<ProblemReportInput, 'adminConfig'> {
  className?: string;
}

export function ProblemReportPanel({
  errorMessage,
  context,
  lastApi,
  extra,
  className,
}: ProblemReportPanelProps) {
  const { t } = useI18n();
  const { data: adminConfig } = useAdminConfig();
  const [copied, setCopied] = useState(false);

  if (!isProblemReportEnabled() || !errorMessage.trim()) return null;

  const input: ProblemReportInput = {
    errorMessage,
    context,
    lastApi,
    extra,
    adminConfig: adminConfig ?? null,
  };

  const copyDiagnostics = async () => {
    const text = buildProblemReportBody(input);
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2500);
    } catch {
      /* clipboard blocked */
    }
  };

  return (
    <div className={className ?? 'mt-2 rounded-md border border-border/80 bg-muted/30 px-3 py-2'}>
      <p className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
        <Bug className="h-3.5 w-3.5 shrink-0" />
        {t('report.hint')}
      </p>
      <div className="mt-2 flex flex-wrap gap-2">
        <Button type="button" variant="outline" size="sm" className="h-8 gap-1.5 text-xs" asChild>
          <a href={buildMailtoUrl(input)}>{t('report.email')}</a>
        </Button>
        <Button type="button" variant="outline" size="sm" className="h-8 gap-1.5 text-xs" asChild>
          <a href={buildGitHubIssueUrl(input)} target="_blank" rel="noopener noreferrer">
            <Github className="h-3.5 w-3.5" />
            {t('report.github')}
          </a>
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-8 gap-1.5 text-xs"
          onClick={() => void copyDiagnostics()}
        >
          <Copy className="h-3.5 w-3.5" />
          {copied ? t('report.copied') : t('report.copy')}
        </Button>
      </div>
    </div>
  );
}
