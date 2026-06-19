#!/usr/bin/env node
/**
 * Validates i18n key coverage for the Ferrum UI.
 * - Every t('key') used in src must exist in en.ts
 * - Overlay keys (de/fr/ar) must be valid paths under en.ts
 * - fr.ts and ar.ts overlays must include every de.ts overlay key (parity)
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const ROOT = join(fileURLToPath(new URL('.', import.meta.url)), '..');
const SRC = join(ROOT, 'src');
const I18N = join(SRC, 'i18n');

function extractObjectLiteral(source, pattern) {
  const match = source.match(pattern);
  if (!match) throw new Error(`Could not parse object: ${pattern}`);
  return vm.runInNewContext(`(${match[1]})`, {}, { timeout: 2000 });
}

function loadEn() {
  const src = readFileSync(join(I18N, 'en.ts'), 'utf8');
  return extractObjectLiteral(src, /export const en = (\{[\s\S]*\})\s*;/);
}

function loadOverlay(filename, constName) {
  const src = readFileSync(join(I18N, filename), 'utf8');
  const re = new RegExp(`(?:export )?const ${constName} = (\\{[\\s\\S]*\\n\\});`);
  return extractObjectLiteral(src, re);
}

function flatten(obj, prefix = '') {
  const out = new Set();
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
      for (const child of flatten(v, key)) out.add(child);
    } else {
      out.add(key);
    }
  }
  return out;
}

function walkTsFiles(dir, files = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) {
      if (name === 'i18n') continue;
      walkTsFiles(p, files);
    } else if (/\.(tsx?)$/.test(name)) {
      files.push(p);
    }
  }
  return files;
}

function collectUsedKeys() {
  const keys = new Set();
  const re = /\bt\(\s*['"`]([^'"`]+)['"`]/g;
  for (const file of walkTsFiles(SRC)) {
    const src = readFileSync(file, 'utf8');
    let m;
    while ((m = re.exec(src)) !== null) {
      if (!m[1].includes('${')) keys.add(m[1]);
    }
  }
  return keys;
}

function reportMissing(missingFrom, reference, label) {
  const gaps = [...reference].filter((k) => !missingFrom.has(k)).sort();
  if (gaps.length) {
    console.error(`\n${label} (${gaps.length} missing):`);
    for (const k of gaps.slice(0, 40)) console.error(`  - ${k}`);
    if (gaps.length > 40) console.error(`  … and ${gaps.length - 40} more`);
    return gaps.length;
  }
  return 0;
}

function reportInvalid(invalid, label) {
  if (invalid.length) {
    console.error(`\n${label} (${invalid.length} invalid):`);
    for (const k of invalid.slice(0, 40)) console.error(`  - ${k}`);
    if (invalid.length > 40) console.error(`  … and ${invalid.length - 40} more`);
    return invalid.length;
  }
  return 0;
}

const en = loadEn();
const deOverlay = loadOverlay('de.ts', 'deOverlay');
const frOverlay = loadOverlay('frOverlay.ts', 'frOverlay');
const arOverlay = loadOverlay('arOverlay.ts', 'arOverlay');

const enKeys = flatten(en);
const deKeys = flatten(deOverlay);
const frKeys = flatten(frOverlay);
const arKeys = flatten(arOverlay);
const usedKeys = collectUsedKeys();

let errors = 0;

errors += reportMissing(enKeys, usedKeys, 'Used t() keys missing from en.ts');

for (const [label, overlayKeys] of [
  ['de.ts', deKeys],
  ['frOverlay.ts', frKeys],
  ['arOverlay.ts', arKeys],
]) {
  const invalid = [...overlayKeys].filter((k) => !enKeys.has(k)).sort();
  errors += reportInvalid(invalid, `${label} overlay keys not in en.ts`);
}

errors += reportMissing(frKeys, deKeys, 'frOverlay.ts missing de keys (parity)');
errors += reportMissing(arKeys, deKeys, 'arOverlay.ts missing de keys (parity)');

if (errors) {
  console.error(`\ni18n check failed with ${errors} issue(s).`);
  process.exit(1);
}

console.log(
  `i18n OK: ${usedKeys.size} used keys, ${enKeys.size} en keys, ${deKeys.size} de overlay keys, fr ${frKeys.size}, ar ${arKeys.size}`,
);
