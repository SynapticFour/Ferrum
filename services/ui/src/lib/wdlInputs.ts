/** Lightweight WDL workflow input parser for dynamic run forms. */

export type WdlPrimitiveType = 'File' | 'String' | 'Int' | 'Float' | 'Boolean' | 'Array' | 'Map' | 'other';

export interface WdlInput {
  name: string;
  wdlType: WdlPrimitiveType;
  optional: boolean;
}

export interface ParsedWdlWorkflow {
  workflowName: string;
  inputs: WdlInput[];
}

const WORKFLOW_RE = /workflow\s+(\w+)\s*\{/i;

function parseInputLine(line: string): WdlInput | null {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith('#')) return null;
  const optional = trimmed.endsWith('?');
  const withoutOpt = optional ? trimmed.slice(0, -1).trim() : trimmed;
  const parts = withoutOpt.split(/\s+/);
  if (parts.length < 2) return null;
  const rawType = parts[0].replace(/\?+$/, '');
  const name = parts[parts.length - 1];
  let wdlType: WdlPrimitiveType = 'other';
  if (rawType.startsWith('File')) wdlType = 'File';
  else if (rawType.startsWith('String')) wdlType = 'String';
  else if (rawType.startsWith('Int')) wdlType = 'Int';
  else if (rawType.startsWith('Float')) wdlType = 'Float';
  else if (rawType.startsWith('Boolean')) wdlType = 'Boolean';
  else if (rawType.startsWith('Array')) wdlType = 'Array';
  else if (rawType.startsWith('Map')) wdlType = 'Map';
  return { name, wdlType, optional };
}

function extractWorkflowInputBlock(wdl: string, workflowName: string): string | null {
  const wfIdx = wdl.search(new RegExp(`workflow\\s+${workflowName}\\s*\\{`, 'i'));
  if (wfIdx < 0) return null;
  const slice = wdl.slice(wfIdx);
  const inputMatch = slice.match(/input\s*\{([^}]*)\}/is);
  return inputMatch?.[1] ?? null;
}

export function parseWdlWorkflowInputs(wdl: string): ParsedWdlWorkflow | null {
  const wfMatch = wdl.match(WORKFLOW_RE);
  if (!wfMatch) return null;
  const workflowName = wfMatch[1];
  const block = extractWorkflowInputBlock(wdl, workflowName);
  if (!block) return { workflowName, inputs: [] };
  const inputs: WdlInput[] = [];
  for (const line of block.split('\n')) {
    const parsed = parseInputLine(line);
    if (parsed) inputs.push(parsed);
  }
  return { workflowName, inputs };
}

/** Cromwell / Ferrum WES flat param key: WorkflowName.input_name */
export function wesParamKey(workflowName: string, inputName: string): string {
  return `${workflowName}.${inputName}`;
}

/** File inputs that typically vary per sample (BAM, FASTQ, …). */
export function isPerSampleFileInput(name: string): boolean {
  const n = name.toLowerCase();
  if (n.includes('ref') || n.includes('truth') || n.includes('dict')) return false;
  if (n.includes('interval')) return false;
  if (n.startsWith('input_')) return true;
  if (n.includes('bam') || n.includes('fastq') || n.includes('cram') || n.includes('vcf')) return true;
  return false;
}

export function defaultValueForInput(input: WdlInput): string {
  if (input.wdlType === 'Int' || input.wdlType === 'Float') return '0';
  if (input.name.toLowerCase().includes('interval')) return 'chr22:1700-2300';
  return '';
}
