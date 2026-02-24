import { useRef, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export function LiveLogViewer({
  lines,
  className,
  maxHeight = '20rem',
}: {
  lines: string[];
  className?: string;
  maxHeight?: string;
}) {
  const preRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    if (preRef.current) preRef.current.scrollTop = preRef.current.scrollHeight;
  }, [lines.length]);

  const handleDownload = () => {
    const blob = new Blob([lines.join('\n')], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'run.log';
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className={cn('rounded-md border border-border bg-card', className)}>
      <div className="flex items-center justify-end border-b border-border px-2 py-1">
        <Button variant="ghost" size="sm" onClick={handleDownload}>Download</Button>
      </div>
      <pre
        ref={preRef}
        className="overflow-auto p-4 font-mono text-xs"
        style={{ maxHeight }}
      >
        {lines.length ? lines.join('\n') : 'No log output yet.'}
      </pre>
    </div>
  );
}
