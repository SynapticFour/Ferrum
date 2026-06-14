/** Parse cohort sample sheets (CSV / TSV). Excel: export as CSV first. */

export interface SampleSheetRow {
  sample_id: string;
  drs_object_ids: string[];
  phenotype: Record<string, string>;
  warnings: string[];
}

interface ParsedSheetRow extends SampleSheetRow {
  drsName?: string;
  drsPath?: string;
}

const KNOWN_PHENOTYPE_COLS = new Set([
  'sex',
  'sequencing_type',
  'diagnosis',
  'ancestry',
  'tissue_type',
  'age',
]);

function splitLine(line: string, delimiter: string): string[] {
  const out: string[] = [];
  let cur = '';
  let inQuotes = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === '"') {
      inQuotes = !inQuotes;
      continue;
    }
    if (!inQuotes && ch === delimiter) {
      out.push(cur.trim());
      cur = '';
      continue;
    }
    cur += ch;
  }
  out.push(cur.trim());
  return out;
}

function detectDelimiter(headerLine: string): string {
  const tabs = (headerLine.match(/\t/g) ?? []).length;
  const commas = (headerLine.match(/,/g) ?? []).length;
  return tabs > commas ? '\t' : ',';
}

function normHeader(h: string): string {
  return h
    .trim()
    .toLowerCase()
    .replace(/\s+/g, '_')
    .replace(/[^a-z0-9_]/g, '');
}

export function parseSampleSheetText(text: string): SampleSheetRow[] {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !l.startsWith('#'));
  if (lines.length < 2) return [];

  const delimiter = detectDelimiter(lines[0]);
  const headers = splitLine(lines[0], delimiter).map(normHeader);
  const sampleIdx = headers.findIndex((h) => h === 'sample_id' || h === 'sample' || h === 'sampleid');
  if (sampleIdx < 0) {
    throw new Error('Sheet must have a sample_id (or sample) column');
  }

  const drsIdIdx = headers.findIndex((h) =>
    ['drs_object_id', 'drs_object_ids', 'drs_id', 'drs_ids', 'data_object_id'].includes(h),
  );
  const drsNameIdx = headers.findIndex((h) =>
    ['drs_name', 'drs_object_name', 'data_name', 'file_name', 'filename'].includes(h),
  );
  const drsPathIdx = headers.findIndex((h) =>
    ['drs_path', 'path', 'file_path', 'uri', 'url'].includes(h),
  );

  const rows: ParsedSheetRow[] = [];
  for (let i = 1; i < lines.length; i++) {
    const cells = splitLine(lines[i], delimiter);
    const sample_id = cells[sampleIdx]?.trim();
    if (!sample_id) continue;

    const warnings: string[] = [];
    const drs_object_ids: string[] = [];
    if (drsIdIdx >= 0 && cells[drsIdIdx]) {
      for (const part of cells[drsIdIdx].split(/[;,|]/)) {
        const id = part.trim();
        if (id) drs_object_ids.push(id);
      }
    }

    const phenotype: Record<string, string> = {};
    headers.forEach((h, idx) => {
      if (idx === sampleIdx || idx === drsIdIdx || idx === drsNameIdx || idx === drsPathIdx) return;
      const val = cells[idx]?.trim();
      if (!val) return;
      if (KNOWN_PHENOTYPE_COLS.has(h) || h.startsWith('phenotype_')) {
        phenotype[h.replace(/^phenotype_/, '')] = val;
      } else if (!['workspace_id', 'cohort_id', 'notes', 'comment'].includes(h)) {
        phenotype[h] = val;
      }
    });

    rows.push({
      sample_id,
      drs_object_ids,
      phenotype,
      warnings,
      drsName: drsNameIdx >= 0 ? cells[drsNameIdx]?.trim() : undefined,
      drsPath: drsPathIdx >= 0 ? cells[drsPathIdx]?.trim() : undefined,
    });
  }
  return rows;
}

export function resolveSheetDrsIds(
  rows: ParsedSheetRow[],
  drsById: Map<string, { id: string; name?: string }>,
  drsByName: Map<string, string>,
): SampleSheetRow[] {
  return rows.map((row) => {
    const ids = [...row.drs_object_ids];
    const warnings = [...row.warnings];
    for (const id of [...ids]) {
      if (!drsById.has(id)) warnings.push(`Unknown DRS id "${id}"`);
    }
    if (row.drsName) {
      const key = row.drsName.toLowerCase();
      const id = drsByName.get(key);
      if (id) ids.push(id);
      else warnings.push(`No DRS object named "${row.drsName}"`);
    }
    if (row.drsPath && !row.drsName) {
      warnings.push(`Path column not auto-linked — register "${row.drsPath}" as DRS first`);
    }
    return { sample_id: row.sample_id, drs_object_ids: [...new Set(ids)], phenotype: row.phenotype, warnings };
  });
}
