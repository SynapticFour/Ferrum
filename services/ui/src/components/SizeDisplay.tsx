import { formatBytes } from '@/lib/utils';

export function SizeDisplay({ bytes }: { bytes: number }) {
  return <span>{formatBytes(bytes)}</span>;
}
