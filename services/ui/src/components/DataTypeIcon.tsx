import { FileText, FileCode, Database, BarChart3, Lock } from 'lucide-react';
import { cn } from '@/lib/utils';

const mimeIcons: Record<string, React.ReactNode> = {
  'application/gzip': <FileCode className="h-4 w-4" />,
  'application/vnd.ga4gh.drs.v1+json': <Database className="h-4 w-4" />,
  'text/vcf': <BarChart3 className="h-4 w-4" />,
  'text/plain': <FileText className="h-4 w-4" />,
};

export function DataTypeIcon({ mimeType, encrypted, className }: { mimeType?: string; encrypted?: boolean; className?: string }) {
  const icon = mimeType && mimeIcons[mimeType] ? mimeIcons[mimeType] : <FileText className="h-4 w-4" />;
  return (
    <span className={cn('inline-flex items-center gap-1', className)}>
      {icon}
      {encrypted && <Lock className="h-3 w-3 text-muted-foreground" />}
    </span>
  );
}
