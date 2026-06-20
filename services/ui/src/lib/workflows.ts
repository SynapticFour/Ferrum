export interface CuratedWorkflow {
  id: string;
  nameKey: string;
  descriptionKey: string;
  workflowUrl: string;
  workflowType: 'WDL' | 'CWL' | 'Nextflow' | 'Snakemake';
  workflowTypeVersion: string;
  paramPrefix: string;
  defaultInterval: string;
}

export const CURATED_WORKFLOWS: CuratedWorkflow[] = [
  {
    id: 'tiny-germline-hc',
    nameKey: 'workflows.curated.tinyGermlineHc',
    descriptionKey: 'workflows.curated.tinyGermlineHcDesc',
    workflowUrl:
      'https://raw.githubusercontent.com/SynapticFour/Ferrum-GA4GH-Demo/main/workflows/tiny_hc.wdl',
    workflowType: 'WDL',
    workflowTypeVersion: '1.0',
    paramPrefix: 'TinyGermlineHC',
    defaultInterval: 'chr22:1700-2300',
  },
  {
    id: 'demo-wdl-hello',
    nameKey: 'workflows.curated.wdlHello',
    descriptionKey: 'workflows.curated.wdlHelloDesc',
    workflowUrl: '/ga4gh/trs/v2/tools/demo-wdl-hello/versions/demo-wdl-hello-1.0/descriptor/WDL',
    workflowType: 'WDL',
    workflowTypeVersion: '1.0',
    paramPrefix: '',
    defaultInterval: '',
  },
  {
    id: 'demo-cwl-sort',
    nameKey: 'workflows.curated.cwlSort',
    descriptionKey: 'workflows.curated.cwlSortDesc',
    workflowUrl: '/ga4gh/trs/v2/tools/demo-cwl-sort/versions/demo-cwl-sort-1.0/descriptor/CWL',
    workflowType: 'CWL',
    workflowTypeVersion: '1.0',
    paramPrefix: '',
    defaultInterval: '',
  },
  {
    id: 'demo-nextflow-qc',
    nameKey: 'workflows.curated.nextflowQc',
    descriptionKey: 'workflows.curated.nextflowQcDesc',
    workflowUrl:
      '/ga4gh/trs/v2/tools/demo-nextflow-qc/versions/demo-nextflow-qc-1.0/descriptor/NFL',
    workflowType: 'Nextflow',
    workflowTypeVersion: 'DSL2',
    paramPrefix: '',
    defaultInterval: '',
  },
  {
    id: 'demo-snakemake-hello',
    nameKey: 'workflows.curated.snakemakeHello',
    descriptionKey: 'workflows.curated.snakemakeHelloDesc',
    workflowUrl: '/ga4gh/trs/v2/tools/demo-snakemake-hello/versions/demo-snakemake-hello-1.0/descriptor/SMK',
    workflowType: 'Snakemake',
    workflowTypeVersion: '7',
    paramPrefix: '',
    defaultInterval: '',
  },
];

export function trsDescriptorUrl(
  toolId: string,
  versionId: string,
  descriptorType = 'WDL',
): string {
  const origin = typeof window !== 'undefined' ? window.location.origin : '';
  return `${origin}/ga4gh/trs/v2/tools/${encodeURIComponent(toolId)}/versions/${encodeURIComponent(versionId)}/descriptor/${descriptorType}`;
}
