import { apiPost } from '@/api/client';
import { federatedWesRunsUrl } from '@/api/access';
import type { DrsObject } from '@/api/types';
import { drsStorageKind } from '@/lib/drsStorage';
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
  if (n.includes('bam') && !n.includes('index')) return ['bam', 'octet-stream', 'vnd.ga4gh.bam'];
  if (n.includes('bai') || (n.includes('bam') && n.includes('index'))) return ['bai', 'octet-stream'];
  if (n.includes('cram')) return ['cram', 'octet-stream'];
  if (n.includes('fai') || (n.includes('ref') && n.includes('index'))) return ['fai', 'octet-stream', 'x-fasta'];
  if (n.includes('tbi') || (n.includes('truth') && n.includes('index'))) return ['tbi', 'octet-stream', 'gzip'];
  if (n.includes('vcf') || n.includes('truth')) return ['vcf', 'gzip', 'text'];
  if (n.includes('fastq') || n.includes('fq')) return ['fastq'];
  if (n.includes('fasta') || n.includes('ref') || n.endsWith('_fa')) return ['fasta', 'x-fasta', 'text'];
  return [];
}

/** Prefer managed workspace objects for shared (non per-sample) workflow File inputs. */
export function pickSharedDrsObjectForInput(
  inputName: string,
  objects: DrsObject[],
): DrsObject | undefined {
  const managed = objects.filter((o) => drsStorageKind(o) !== 'url');
  if (!managed.length) return undefined;

  const hints = mimeHintsForInput(inputName);
  const nameL = inputName.toLowerCase();
  for (const hint of hints) {
    const hit = managed.find(
      (o) =>
        o.mime_type?.toLowerCase().includes(hint) ||
        o.name?.toLowerCase().includes(hint) ||
        o.id.toLowerCase().includes(hint),
    );
    if (hit) return hit;
  }

  const pilotHints: Array<[RegExp, RegExp]> = [
    [/ref_fasta_index|ref.*index/, /reference fasta index|\.fai/i],
    [/ref_fasta|ref/, /reference fasta|pilot-ref/i],
    [/truth.*index/, /truth vcf index|\.tbi/i],
    [/truth.*vcf|truth/, /truth vcf/i],
  ];
  for (const [inputPat, namePat] of pilotHints) {
    if (!inputPat.test(nameL)) continue;
    const hit = managed.find((o) => namePat.test(o.name ?? ''));
    if (hit) return hit;
  }

  return undefined;
}

export function resolveSharedFileParams(
  workflowName: string,
  fileInputs: WdlInput[],
  objects: DrsObject[],
): Record<string, string> {
  const params: Record<string, string> = {};
  for (const input of fileInputs) {
    if (input.wdlType !== 'File' || isPerSampleFileInput(input.name)) continue;
    const obj = pickSharedDrsObjectForInput(input.name, objects);
    if (obj) params[wesParamKey(workflowName, input.name)] = drsStreamUrl(obj.id);
  }
  return params;
}

/** Form-friendly shared File values keyed by WDL input name (not flattened WES keys). */
export function resolveSharedFileFormValues(
  fileInputs: WdlInput[],
  objects: DrsObject[],
): { values: Record<string, string>; labels: Record<string, string> } {
  const values: Record<string, string> = {};
  const labels: Record<string, string> = {};
  for (const input of fileInputs) {
    if (input.wdlType !== 'File' || isPerSampleFileInput(input.name)) continue;
    const obj = pickSharedDrsObjectForInput(input.name, objects);
    if (obj) {
      values[input.name] = drsStreamUrl(obj.id);
      labels[input.name] = obj.name ?? obj.id;
    }
  }
  return { values, labels };
}

export function pickDrsObjectForInput(
  inputName: string,
  objectIds: string[],
  lookup: Map<string, DrsObject>,
): DrsObject | undefined {
  const hints = mimeHintsForInput(inputName);
  const candidates = objectIds
    .map((id) => lookup.get(id))
    .filter((o): o is DrsObject => !!o && drsStorageKind(o) !== 'url');
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
