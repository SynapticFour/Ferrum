import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/api/client';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
  Line,
  ComposedChart,
} from 'recharts';
import { Button } from '@/components/ui/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { useMemo, useState } from 'react';
import { Download } from 'lucide-react';
import { useI18n } from '@/i18n/I18nProvider';

interface EstimatedCost {
  amount: number;
  currency: string;
}

interface RunMetricsSummary {
  wall_time: string;
  total_cpu_seconds: number;
  peak_memory_mb: number;
  total_read_gb: number;
  total_write_gb: number;
  estimated_cost: EstimatedCost;
}

interface RunMetricsTask {
  name: string;
  wall_seconds: number;
  cpu_peak_pct: number;
  memory_peak_mb: number;
  exit_code: number | null;
}

interface RunMetricsTimeseries {
  timestamps: string[];
  cpu_pct: number[];
  memory_mb: number[];
}

interface RunMetricsResponse {
  run_id: string;
  summary: RunMetricsSummary;
  tasks: RunMetricsTask[];
  timeseries: RunMetricsTimeseries;
}

function formatBytes(n: number): string {
  if (n >= 1024) return `${(n / 1024).toFixed(1)} GB`;
  return `${n} MB`;
}

export function RunMetricsTab({ runId }: { runId: string }) {
  const { t } = useI18n();
  const [taskSort, setTaskSort] = useState<'name' | 'duration' | 'memory'>('duration');
  const { data, isLoading, error } = useQuery({
    queryKey: ['wes', 'metrics', runId],
    queryFn: () =>
      apiGet<RunMetricsResponse>(`/ga4gh/wes/v1/runs/${encodeURIComponent(runId)}/metrics`),
    enabled: !!runId,
  });

  const sortedTasks = useMemo(() => {
    if (!data?.tasks) return [];
    const list = [...data.tasks];
    if (taskSort === 'duration') list.sort((a, b) => b.wall_seconds - a.wall_seconds);
    else if (taskSort === 'memory') list.sort((a, b) => b.memory_peak_mb - a.memory_peak_mb);
    else list.sort((a, b) => a.name.localeCompare(b.name));
    return list;
  }, [data?.tasks, taskSort]);

  const timeseriesChartData = useMemo(() => {
    if (!data?.timeseries) return [];
    const { timestamps, cpu_pct, memory_mb } = data.timeseries;
    return timestamps.map((ts, i) => ({
      time: ts.slice(11, 19),
      cpu: cpu_pct[i] ?? 0,
      memory: memory_mb[i] ?? 0,
    }));
  }, [data?.timeseries]);

  const exportCsv = () => {
    if (!data) return;
    const headers = ['Task', 'Duration (s)', 'CPU-Hours', 'Memory GB·h', 'Peak Memory (MB)', 'Exit Code'];
    const rows = sortedTasks.map((t) => {
      const cpuH = (t.wall_seconds / 3600);
      const memGbH = (t.wall_seconds / 3600) * (t.memory_peak_mb / 1024);
      return [t.name, t.wall_seconds, cpuH.toFixed(4), memGbH.toFixed(4), t.memory_peak_mb, t.exit_code ?? '—'];
    });
    const csv = [headers.join(','), ...rows.map((r) => r.join(','))].join('\n');
    const blob = new Blob([csv], { type: 'text/csv' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `run-${runId}-metrics.csv`;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  if (isLoading) return <p className="text-muted-foreground">{t('run.metricsLoading')}</p>;
  if (error) return <p className="text-destructive">{t('run.metricsUnavailable')}</p>;
  if (!data) return null;

  const { summary, tasks } = data;
  const cpuHours = (summary.total_cpu_seconds / 3600).toFixed(2);

  return (
    <div className="space-y-6">
      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-1"><CardTitle className="text-sm font-medium">{t('run.metricsWallTime')}</CardTitle></CardHeader>
          <CardContent><div className="text-xl font-semibold">{summary.wall_time}</div></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-1"><CardTitle className="text-sm font-medium">{t('run.metricsCpuHours')}</CardTitle></CardHeader>
          <CardContent><div className="text-xl font-semibold">{cpuHours}</div></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-1"><CardTitle className="text-sm font-medium">{t('run.metricsPeakMemory')}</CardTitle></CardHeader>
          <CardContent><div className="text-xl font-semibold">{formatBytes(summary.peak_memory_mb)}</div></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-1"><CardTitle className="text-sm font-medium">{t('run.metricsEstCost')}</CardTitle></CardHeader>
          <CardContent><div className="text-xl font-semibold">{summary.estimated_cost.currency} {summary.estimated_cost.amount.toFixed(2)}</div></CardContent>
        </Card>
      </div>

      {tasks.length > 0 && (
        <Card>
          <CardHeader><CardTitle>{t('run.metricsTaskTimeline')}</CardTitle></CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={Math.min(400, tasks.length * 28 + 80)}>
              <BarChart
                data={sortedTasks.map((t) => ({ name: t.name.length > 24 ? t.name.slice(0, 21) + '…' : t.name, seconds: t.wall_seconds }))}
                layout="vertical"
                margin={{ left: 8, right: 24 }}
              >
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis type="number" unit="s" />
                <YAxis type="category" dataKey="name" width={140} tick={{ fontSize: 11 }} />
                <Tooltip formatter={(v: number) => [`${v} s`, t('run.metricsDuration')]} />
                <Bar dataKey="seconds" fill="hsl(var(--primary))" name={t('run.metricsDuration')} radius={[0, 2, 2, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      )}

      {timeseriesChartData.length > 0 && (
        <Card>
          <CardHeader><CardTitle>{t('run.metricsResourceUsage')}</CardTitle></CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={280}>
              <ComposedChart data={timeseriesChartData}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="time" tick={{ fontSize: 10 }} />
                <YAxis yAxisId="cpu" orientation="left" tick={{ fontSize: 10 }} label={{ value: t('run.metricsCpuPct'), angle: -90, position: 'insideLeft' }} />
                <YAxis yAxisId="mem" orientation="right" tick={{ fontSize: 10 }} label={{ value: t('run.metricsMemoryMb'), angle: 90, position: 'insideRight' }} />
                <Tooltip />
                <Legend />
                <Line yAxisId="cpu" type="monotone" dataKey="cpu" stroke="#0ea5e9" name={t('run.metricsCpuPct')} dot={false} />
                <Line yAxisId="mem" type="monotone" dataKey="memory" stroke="#10b981" name={t('run.metricsMemoryMb')} dot={false} />
              </ComposedChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>{t('run.metricsPerTaskCost')}</CardTitle>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => setTaskSort('duration')}>{t('run.metricsByDuration')}</Button>
            <Button variant="outline" size="sm" onClick={() => setTaskSort('memory')}>{t('run.metricsByMemory')}</Button>
            <Button variant="outline" size="sm" onClick={() => setTaskSort('name')}>{t('run.metricsByName')}</Button>
            <Button variant="outline" size="sm" onClick={exportCsv}><Download className="h-4 w-4 mr-1" /> CSV</Button>
          </div>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t('run.metricsColTask')}</TableHead>
                <TableHead>{t('run.metricsColDuration')}</TableHead>
                <TableHead>{t('run.metricsColCpuHours')}</TableHead>
                <TableHead>{t('run.metricsColMemory')}</TableHead>
                <TableHead>{t('run.metricsColExit')}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {sortedTasks.map((t) => (
                <TableRow key={t.name}>
                  <TableCell className="font-medium">{t.name}</TableCell>
                  <TableCell>{t.wall_seconds}s</TableCell>
                  <TableCell>{(t.wall_seconds / 3600).toFixed(4)}</TableCell>
                  <TableCell>{((t.wall_seconds / 3600) * (t.memory_peak_mb / 1024)).toFixed(4)}</TableCell>
                  <TableCell>{t.exit_code != null ? t.exit_code : '—'}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          {sortedTasks.length === 0 && <p className="text-muted-foreground text-sm py-4">{t('run.metricsNoTasks')}</p>}
        </CardContent>
      </Card>
    </div>
  );
}
