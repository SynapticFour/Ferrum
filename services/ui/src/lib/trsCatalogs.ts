/** Public TRS-enabled catalogs and one-click import presets for the Tool Registry UI. */

export interface TrsExternalCatalog {
  id: string;
  name: string;
  url: string;
  descriptionKey: string;
  trsBaseUrl?: string;
}

export interface TrsImportPreset {
  id: string;
  nameKey: string;
  workflowUrl: string;
  engineId: string;
  toolclass: string;
  sourceKey: string;
}

export const TRS_TOOL_CLASSES = [
  { id: 'Workflow', labelKey: 'tools.classWorkflow' },
  { id: 'Command-line tool', labelKey: 'tools.classCommandLine' },
  { id: 'Software', labelKey: 'tools.classSoftware' },
  { id: 'Container', labelKey: 'tools.classContainer' },
] as const;

export const DOCKSTORE_TRS_BASE = 'https://dockstore.org/api/ga4gh/trs/v2';

export const TRS_EXTERNAL_CATALOGS: TrsExternalCatalog[] = [
  {
    id: 'dockstore',
    name: 'Dockstore',
    url: 'https://dockstore.org/',
    descriptionKey: 'tools.catalogDockstore',
    trsBaseUrl: DOCKSTORE_TRS_BASE,
  },
  {
    id: 'workflowhub',
    name: 'WorkflowHub',
    url: 'https://workflowhub.eu/',
    descriptionKey: 'tools.catalogWorkflowHub',
  },
  {
    id: 'nfcore',
    name: 'nf-core',
    url: 'https://nf-co.re/pipelines',
    descriptionKey: 'tools.catalogNfcore',
  },
];

export const TRS_IMPORT_PRESETS: TrsImportPreset[] = [
  {
    id: 'ferrum-tiny-hc',
    nameKey: 'tools.presetTinyHc',
    workflowUrl:
      'https://raw.githubusercontent.com/SynapticFour/Ferrum-GA4GH-Demo/main/workflows/tiny_hc.wdl',
    engineId: 'wdl',
    toolclass: 'Workflow',
    sourceKey: 'tools.presetSourceFerrum',
  },
  {
    id: 'nfcore-fetchngs',
    nameKey: 'tools.presetFetchngs',
    workflowUrl: 'https://raw.githubusercontent.com/nf-core/fetchngs/master/main.nf',
    engineId: 'nextflow',
    toolclass: 'Workflow',
    sourceKey: 'tools.presetSourceNfcore',
  },
];

export interface RegisterToolPreset {
  name?: string;
  description?: string;
  workflowUrl?: string;
  workflowContent?: string;
  engineId?: string;
  toolclass?: string;
}

export function descriptorTypeToEngineId(descriptorType: string): string | undefined {
  switch (descriptorType.toUpperCase()) {
    case 'WDL':
      return 'wdl';
    case 'CWL':
      return 'cwl';
    case 'NFL':
    case 'NXF':
      return 'nextflow';
    case 'SMK':
      return 'snakemake';
    default:
      return undefined;
  }
}

export function dockstoreToolclassToFerrum(name?: string): string {
  if (name === 'Workflow') return 'Workflow';
  if (name === 'CommandLineTool') return 'Command-line tool';
  return 'Software';
}
