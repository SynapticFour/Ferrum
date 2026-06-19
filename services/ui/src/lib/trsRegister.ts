import { apiPost } from '@/api/client';
import { trsDescriptorUrl } from '@/lib/workflows';
import { WORKFLOW_ENGINES } from '@/lib/workflowEngines';

export interface TrsRegisteredTool {
  id: string;
  name?: string;
  versions?: Array<{ id: string; name?: string }>;
}

export interface RegisterWorkflowInTrsOptions {
  name?: string;
  description?: string;
  workflowUrl?: string;
  workflowContent?: string;
  workflowType: string;
  workflowTypeVersion?: string;
}

export function descriptorTypeForWesType(wesType: string): string {
  const eng = WORKFLOW_ENGINES.find((e) => e.wesType.toLowerCase() === wesType.toLowerCase());
  return eng?.trsDescriptor ?? 'WDL';
}

export function isTrsDescriptorUrl(url: string): boolean {
  return url.includes('/ga4gh/trs/v2/tools/') && url.includes('/descriptor');
}

export async function registerWorkflowInTrs(
  opts: RegisterWorkflowInTrsOptions,
): Promise<{ toolId: string; versionId: string; descriptorUrl: string; descriptorType: string }> {
  const workflowUrl = opts.workflowUrl?.trim();
  const workflowContent = opts.workflowContent?.trim();
  if (!workflowUrl && !workflowContent) {
    throw new Error('workflow_url or workflow_content required');
  }

  const tool = await apiPost<TrsRegisteredTool>('/ga4gh/trs/v2/internal/register', {
    name: opts.name?.trim() || undefined,
    description: opts.description?.trim() || undefined,
    organization: 'Ferrum',
    toolclass: 'Workflow',
    workflow_type: opts.workflowType,
    workflow_type_version: opts.workflowTypeVersion ?? '1.0',
    workflow_url: workflowUrl || undefined,
    workflow_content: workflowContent || undefined,
  });

  const version = tool.versions?.[0];
  if (!version?.id) {
    throw new Error('TRS register succeeded but no version returned');
  }

  const descriptorType = descriptorTypeForWesType(opts.workflowType);
  return {
    toolId: tool.id,
    versionId: version.id,
    descriptorType,
    descriptorUrl: trsDescriptorUrl(tool.id, version.id, descriptorType),
  };
}
