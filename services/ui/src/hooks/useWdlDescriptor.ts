import { useQuery } from '@tanstack/react-query';
import { useAuthStore } from '@/stores/auth';
import { parseWdlWorkflowInputs, type ParsedWdlWorkflow } from '@/lib/wdlInputs';

async function fetchDescriptorText(url: string): Promise<string> {
  const jwt = useAuthStore.getState().passportJwt;
  const res = await fetch(url, {
    headers: jwt ? { Authorization: `Bearer ${jwt}` } : {},
  });
  if (!res.ok) throw new Error(`Descriptor fetch failed: HTTP ${res.status}`);
  const text = await res.text();
  try {
    const json = JSON.parse(text) as { content?: string; url?: string };
    if (typeof json.content === 'string' && json.content.trim()) return json.content;
    if (typeof json.url === 'string' && json.url.startsWith('http')) {
      const r2 = await fetch(json.url);
      if (!r2.ok) throw new Error(`Remote descriptor URL failed: HTTP ${r2.status}`);
      return r2.text();
    }
  } catch {
    /* plain WDL */
  }
  return text;
}

function resolveDescriptorUrl(workflowUrl: string): string {
  if (workflowUrl.startsWith('http://') || workflowUrl.startsWith('https://')) return workflowUrl;
  if (workflowUrl.startsWith('/')) {
    const origin = typeof window !== 'undefined' ? window.location.origin : '';
    return `${origin}${workflowUrl}`;
  }
  return workflowUrl;
}

export function useWdlDescriptor(workflowUrl: string, workflowType: string, enabled: boolean) {
  const isWdl = workflowType.toLowerCase().includes('wdl');
  return useQuery({
    queryKey: ['wdl-descriptor', workflowUrl, workflowType],
    queryFn: async (): Promise<ParsedWdlWorkflow | null> => {
      if (!workflowUrl.trim()) return null;
      const text = await fetchDescriptorText(resolveDescriptorUrl(workflowUrl));
      return parseWdlWorkflowInputs(text);
    },
    enabled: enabled && isWdl && workflowUrl.trim().length > 0,
    staleTime: 60_000,
    retry: false,
  });
}
