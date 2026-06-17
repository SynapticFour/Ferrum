import { apiPost } from '@/api/client';
import { federatedWesRunsUrl } from '@/api/access';
import type { DrsObject } from '@/api/types';
import { isPerSampleFileInput, wesParamKey, type WdlInput } from '@/lib/wdlInputs';

export function drsStreamUrl(objectId: string): string {
  const origin = typeof window !== 'undefined' ? window.location.origin : '';
  return `${origin}/ga4gh/drs/v1/objects/${encodeURIComponent(objectId)}/stream`;
}

export interface SubmitWorkflowRunOptions {
  workflowType: string;
  workflowTypeVersion?: string;
  workflowUrl: string;
  workflowParams: Record<string, string>;
  workspaceId?: string | null;
  tags?: Record<string, string>;
}

export async function submitWorkflowRun(opts: SubmitWorkflowRunOptions): Promise<{ run_id?: string }> {
  return apiPost<{ run_id?: string }>('/ga4gh/wes/v1/runs', {
    workflow_type: opts.workflowType,
    workflow_type_version: opts.workflowTypeVersion ?? '1.0',
    workflow_url: opts.workflowUrl,
    workflow_params: opts.workflowParams,
    tags: opts.tags ?? { source: 'ferrum-ui' },
    ...(opts.workspaceId ? { workspace_id: opts.workspaceId } : {}),
  });
}

export interface SubmitFederatedWorkflowRunOptions extends SubmitWorkflowRunOptions {
  computePoolId: string;
  remoteWesBaseUrl?: string;
  federationOrigin?: string;
  adsBaseUrl?: string;
}

/** Submit a workflow run on a remote compute pool via the federated WES proxy (or local WES auto-forward). */
export async function submitFederatedWorkflowRun(
  opts: SubmitFederatedWorkflowRunOptions,
): Promise<{ run_id?: string }> {
  const federatedUrl = federatedWesRunsUrl(
    opts.remoteWesBaseUrl,
    opts.federationOrigin,
    opts.computePoolId,
    opts.adsBaseUrl,
  );
  const tags: Record<string, string> = {
    ...(opts.tags ?? {}),
    ads_compute_pool_id: opts.computePoolId,
    source: 'ferrum-ui',
  };
  if (opts.federationOrigin) tags.federation_origin = opts.federationOrigin;
  if (opts.adsBaseUrl) tags.ads_base_url = opts.adsBaseUrl;
  if (opts.remoteWesBaseUrl) tags.remote_wes_base_url = opts.remoteWesBaseUrl;

  const body = {
    workflow_type: opts.workflowType,
    workflow_type_version: opts.workflowTypeVersion ?? '1.0',
    workflow_url: opts.workflowUrl,
    workflow_params: opts.workflowParams,
    tags,
    ...(opts.workspaceId ? { workspace_id: opts.workspaceId } : {}),
  };

  if (federatedUrl) {
    return apiPost<{ run_id?: string }>(federatedUrl, body);
  }
  return apiPost<{ run_id?: string }>('/ga4gh/wes/v1/runs', body);
}

export function buildFlatWorkflowParams(
  workflowName: string,
  values: Record<string, string>,
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [name, value] of Object.entries(values)) {
    if (!value.trim()) continue;
    out[wesParamKey(workflowName, name)] = value.trim();
  }
  return out;
}

function mimeHintsForInput(name: string): string[] {
  const n = name.toLowerCase();
  if (n.includes('bam')) return ['bam', 'octet-stream'];
  if (n.includes('cram')) return ['cram', 'octet-stream'];
  if (n.includes('vcf')) return ['vcf', 'text'];
  if (n.includes('fastq') || n.includes('fq')) return ['fastq'];
  if (n.includes('fasta') || n.includes('fa')) return ['fasta', 'text'];
  return [];
}

export function pickDrsObjectForInput(
  inputName: string,
  objectIds: string[],
  lookup: Map<string, DrsObject>,
): DrsObject | undefined {
  const hints = mimeHintsForInput(inputName);
  const candidates = objectIds.map((id) => lookup.get(id)).filter((o): o is DrsObject => !!o);
  for (const hint of hints) {
    const hit = candidates.find(
      (o) =>
        o.mime_type?.toLowerCase().includes(hint) ||
        o.name?.toLowerCase().includes(hint) ||
        o.id.toLowerCase().includes(hint),
    );
    if (hit) return hit;
  }
  return candidates[0];
}

export function resolvePerSampleFileParams(
  workflowName: string,
  fileInputs: WdlInput[],
  objectIds: string[],
  lookup: Map<string, DrsObject>,
): Record<string, string> {
  const params: Record<string, string> = {};
  for (const input of fileInputs) {
    if (!isPerSampleFileInput(input.name)) continue;
    const obj = pickDrsObjectForInput(input.name, objectIds, lookup);
    if (obj) {
      params[wesParamKey(workflowName, input.name)] = drsStreamUrl(obj.id);
    }
  }
  return params;
}
