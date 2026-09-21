import { describe, expect, it } from 'vitest';

import bgBG from '../bg-BG';
import csCZ from '../cs-CZ';
import daDK from '../da-DK';
import deDE from '../de-DE';
import elGR from '../el-GR';
import en from '../en';
import esES from '../es-ES';
import etEE from '../et-EE';
import fiFI from '../fi-FI';
import frFR from '../fr-FR';
import gaIE from '../ga-IE';
import hrHR from '../hr-HR';
import huHU from '../hu-HU';
import itIT from '../it-IT';
import { SUPPORTED_LOCALES } from '../locales';
import ltLT from '../lt-LT';
import lvLV from '../lv-LV';
import mtMT from '../mt-MT';
import nbNO from '../nb-NO';
import nlNL from '../nl-NL';
import plPL from '../pl-PL';
import ptPT from '../pt-PT';
import roRO from '../ro-RO';
import skSK from '../sk-SK';
import slSI from '../sl-SI';
import svSE from '../sv-SE';
import type { TranslationMap } from '../types';

const CATALOGS: Record<string, TranslationMap> = {
  'bg-BG': bgBG,
  'cs-CZ': csCZ,
  'da-DK': daDK,
  'de-DE': deDE,
  'el-GR': elGR,
  'es-ES': esES,
  'et-EE': etEE,
  'fi-FI': fiFI,
  'fr-FR': frFR,
  'ga-IE': gaIE,
  'hr-HR': hrHR,
  'hu-HU': huHU,
  'it-IT': itIT,
  'lt-LT': ltLT,
  'lv-LV': lvLV,
  'mt-MT': mtMT,
  'nb-NO': nbNO,
  'nl-NL': nlNL,
  'pl-PL': plPL,
  'pt-PT': ptPT,
  'ro-RO': roRO,
  'sk-SK': skSK,
  'sl-SI': slSI,
  'sv-SE': svSE,
};

const EN_KEYS = Object.keys(en);
const NON_DEFAULT_LOCALES = SUPPORTED_LOCALES.map(l => l.code).filter(code => code !== 'en-GB');

const PLURAL_BLOCK = /\{(\w+),\s*plural,\s*((?:\w+\{(?:[^{}]|\{[^{}]*\})*\}\s*)+)\}/;
const PLURAL_CATEGORY = /(\w+)\{((?:[^{}]|\{[^{}]*\})*)\}/g;

/**
 * Replaces each ICU plural block with the concatenation of its category bodies,
 * so category names like `other{s}` are not mistaken for placeholders while
 * real placeholders inside the bodies are kept.
 */
function stripPluralWrappers(value: string): string {
  return value.replace(PLURAL_BLOCK, (_match, _countParam, categoriesRaw: string) => {
    let bodies = '';
    let categoryMatch: RegExpExecArray | null;
    PLURAL_CATEGORY.lastIndex = 0;
    while ((categoryMatch = PLURAL_CATEGORY.exec(categoriesRaw)) !== null) {
      bodies += categoryMatch[2];
    }
    return bodies;
  });
}

/** Sorted `{name}`/`${name}` placeholder tokens in a value. */
function placeholders(value: string): string[] {
  const tokens = new Set<string>();
  for (const match of stripPluralWrappers(value).matchAll(/\$\{\w+\}|\{\w+\}/g)) {
    tokens.add(match[0]);
  }
  return [...tokens].sort();
}

/** Plural category names (`one`, `other`, ...) in order of appearance. */
function pluralCategories(value: string): string[] {
  const block = PLURAL_BLOCK.exec(value);
  if (!block) return [];
  return [...block[2].matchAll(/(\w+)\{/g)].map(m => m[1]);
}

describe('locale catalogs', () => {
  it('covers every supported locale', () => {
    expect(Object.keys(CATALOGS).sort()).toEqual([...NON_DEFAULT_LOCALES].sort());
  });

  it.each(NON_DEFAULT_LOCALES)('%s defines every en-GB key with no extras', code => {
    const catalog = CATALOGS[code];
    expect(catalog).toBeDefined();

    const missing = EN_KEYS.filter(key => catalog[key] === undefined);
    expect(missing).toEqual([]);

    const extra = Object.keys(catalog).filter(key => en[key] === undefined);
    expect(extra).toEqual([]);
  });

  it.each(NON_DEFAULT_LOCALES)('%s preserves every placeholder and plural shape', code => {
    const catalog = CATALOGS[code];
    const mismatched = EN_KEYS.filter(
      key =>
        JSON.stringify(placeholders(en[key])) !==
          JSON.stringify(placeholders(catalog[key] ?? '')) ||
        JSON.stringify(pluralCategories(en[key])) !==
          JSON.stringify(pluralCategories(catalog[key] ?? ''))
    );
    expect(mismatched).toEqual([]);
  });

  it('keeps English text byte-for-byte on the source catalog', () => {
    expect(en['nav.home']).toBe('Home');
    expect(en['settings.language']).toBe('Language');
  });
});
