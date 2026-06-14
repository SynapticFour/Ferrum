/** GA4GH WES workflow_type values supported by Ferrum (see docs/WES-WORKFLOW-ENGINES.md). */
export interface WorkflowEngine {
  id: string;
  wesType: string;
  trsDescriptor: string;
  labelKey: string;
  hintKey: string;
  extensions: string[];
}

export const WORKFLOW_ENGINES: WorkflowEngine[] = [
  {
    id: 'wdl',
    wesType: 'WDL',
    trsDescriptor: 'WDL',
    labelKey: 'engines.wdl',
    hintKey: 'engines.wdlHint',
    extensions: ['.wdl'],
  },
  {
    id: 'cwl',
    wesType: 'CWL',
    trsDescriptor: 'CWL',
    labelKey: 'engines.cwl',
    hintKey: 'engines.cwlHint',
    extensions: ['.cwl', '.yaml', '.yml'],
  },
  {
    id: 'nextflow',
    wesType: 'Nextflow',
    trsDescriptor: 'NFL',
    labelKey: 'engines.nextflow',
    hintKey: 'engines.nextflowHint',
    extensions: ['.nf', '.nf.groovy'],
  },
  {
    id: 'snakemake',
    wesType: 'Snakemake',
    trsDescriptor: 'SMK',
    labelKey: 'engines.snakemake',
    hintKey: 'engines.snakemakeHint',
    extensions: ['.smk', 'Snakefile'],
  },
];

export function engineByWesType(type: string): WorkflowEngine | undefined {
  const lower = type.toLowerCase();
  return WORKFLOW_ENGINES.find(
    (e) =>
      e.wesType.toLowerCase() === lower ||
      e.id === lower ||
      (lower === 'nxf' && e.id === 'nextflow') ||
      (lower === 'nfl' && e.id === 'nextflow') ||
      (lower === 'smk' && e.id === 'snakemake'),
  );
}

export function guessEngineFromFilename(name: string): WorkflowEngine | undefined {
  const lower = name.toLowerCase();
  if (lower.endsWith('.wdl')) return engineByWesType('WDL');
  if (lower.endsWith('.cwl') || lower.endsWith('.yaml') || lower.endsWith('.yml')) return engineByWesType('CWL');
  if (lower.endsWith('.nf') || lower.endsWith('.nf.groovy')) return engineByWesType('Nextflow');
  if (lower.endsWith('.smk') || lower === 'snakefile') return engineByWesType('Snakemake');
  return undefined;
}
