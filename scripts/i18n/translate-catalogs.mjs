// Machine-translation pipeline for the Aura locale catalogs.
//
// Reads the English source catalog (`app/src/lib/i18n/en.ts`), preserves any
// existing hand-written translations already present in a locale file, and
// fills the remaining keys by calling the public Google translate endpoint.
//
// Placeholders (`{name}`), ICU-lite plural blocks, and a small brand/protocol
// allowlist are protected with private-use sentinels before translation so the
// engine cannot mangle them, then restored afterwards.
//
// Usage:
//   node scripts/i18n/translate-catalogs.mjs                 # all locales
//   node scripts/i18n/translate-catalogs.mjs fr-FR it-IT     # only these
//
// Output is raw (unformatted) TS; run `pnpm exec prettier --write` afterwards.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const i18nDir = resolve(repoRoot, 'app', 'src', 'lib', 'i18n');

/** BCP 47 tag -> Google translate language code. */
const GOOGLE_CODE = {
  'bg-BG': 'bg',
  'cs-CZ': 'cs',
  'hr-HR': 'hr',
  'da-DK': 'da',
  'et-EE': 'et',
  'fi-FI': 'fi',
  'fr-FR': 'fr',
  'el-GR': 'el',
  'ga-IE': 'ga',
  'it-IT': 'it',
  'lv-LV': 'lv',
  'lt-LT': 'lt',
  'mt-MT': 'mt',
  'nl-NL': 'nl',
  'pl-PL': 'pl',
  'pt-PT': 'pt',
  'ro-RO': 'ro',
  'sk-SK': 'sk',
  'sl-SI': 'sl',
  'es-ES': 'es',
  'sv-SE': 'sv',
  'de-DE': 'de',
  'hu-HU': 'hu',
  'nb-NO': 'no',
};

const ENGLISH_NAME = {
  'bg-BG': 'Bulgarian',
  'cs-CZ': 'Czech',
  'hr-HR': 'Croatian',
  'da-DK': 'Danish',
  'et-EE': 'Estonian',
  'fi-FI': 'Finnish',
  'fr-FR': 'French',
  'el-GR': 'Greek',
  'ga-IE': 'Irish',
  'it-IT': 'Italian',
  'lv-LV': 'Latvian',
  'lt-LT': 'Lithuanian',
  'mt-MT': 'Maltese',
  'nl-NL': 'Dutch',
  'pl-PL': 'Polish',
  'pt-PT': 'Portuguese',
  'ro-RO': 'Romanian',
  'sk-SK': 'Slovak',
  'sl-SI': 'Slovenian',
  'es-ES': 'Spanish',
  'sv-SE': 'Swedish',
  'de-DE': 'German',
  'hu-HU': 'Hungarian',
  'nb-NO': 'Norwegian Bokmal',
};

const PUBLIC_USE_OPEN = '\uE000';
const PUBLIC_USE_CLOSE = '\uE001';
const SENTINEL_RE = new RegExp(`${PUBLIC_USE_OPEN}(\\d+)${PUBLIC_USE_CLOSE}`, 'g');

const BRANDS = [
  'Aura',
  'GitHub',
  'GitLab',
  'Google',
  'Slack',
  'Discord',
  'Telegram',
  'WhatsApp',
  'iMessage',
  'OpenAI',
  'Anthropic',
  'Claude',
  'Gemini',
  'GPT',
  'macOS',
  'iOS',
  'Android',
  'Windows',
  'Linux',
  'OAuth',
  'MCP',
  'API',
  'URL',
  'HTTPS',
  'JSON',
  'CSV',
  'PDF',
  'QR',
  'PIN',
  'SMS',
  'OTP',
  '2FA',
  'Tauri',
  'Composio',
  'Whisper',
  'Piper',
  'ONNX',
  'LLM',
  'TTS',
  'STT',
  'Meet',
  'Zoom',
];

const CATEGORY_BODY = '(?:[^{}]|\\{[^{}]*\\})*';
const PLURAL_BLOCK = new RegExp(`\\{(\\w+),\\s*plural,\\s*((?:\\w+\\{${CATEGORY_BODY}\\}\\s*)+)\\}`);
const PLURAL_CATEGORY = new RegExp(`(\\w+)\\{(${CATEGORY_BODY})\\}`, 'g');

function parseCatalogSource(source) {
  const assignment = /=\s*\{/.exec(source);
  const start = assignment ? assignment.index + assignment[0].length - 1 : source.indexOf('{');
  const end = source.lastIndexOf('}');
  if (start === -1 || end === -1 || end <= start) {
    throw new Error('could not locate catalog object literal');
  }
  const body = source.slice(start, end + 1);
  // The catalog body is a plain object literal of string -> string; evaluating
  // it keeps us independent of a TypeScript toolchain.
  const obj = new Function(`return (${body});`)();
  return obj;
}

function loadCatalog(fileName) {
  const path = resolve(i18nDir, fileName);
  if (!existsSync(path)) return {};
  return parseCatalogSource(readFileSync(path, 'utf8'));
}

function camelVarName(code) {
  const [lang, region] = code.split('-');
  return `${lang}${region ? region.toUpperCase() : ''}`;
}

function protect(text, store) {
  // `${name}` template placeholders first so the leading `$` is protected with
  // the braces (a lone `$` makes some engines rewrite the token as currency).
  let out = text.replace(/\$\{(\w+)\}/g, match => {
    const index = store.length;
    store.push(match);
    return `${PUBLIC_USE_OPEN}${index}${PUBLIC_USE_CLOSE}`;
  });
  out = out.replace(/\{(\w+)\}/g, match => {
    const index = store.length;
    store.push(match);
    return `${PUBLIC_USE_OPEN}${index}${PUBLIC_USE_CLOSE}`;
  });
  for (const brand of BRANDS) {
    const re = new RegExp(`\\b${brand.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'g');
    out = out.replace(re, () => {
      const index = store.length;
      store.push(brand);
      return `${PUBLIC_USE_OPEN}${index}${PUBLIC_USE_CLOSE}`;
    });
  }
  return out;
}

function restore(text, store) {
  return text.replace(SENTINEL_RE, (match, rawIndex) => store[Number(rawIndex)] ?? match);
}

/**
 * Replaces each ICU plural block with the concatenation of its category bodies,
 * dropping the `{count, plural, one{…} other{…}}` wrappers. Category names like
 * `other{s}` are not placeholders, but real placeholders inside the bodies stay.
 */
function stripPluralWrappers(value) {
  return value.replace(PLURAL_BLOCK, (_match, _countParam, categoriesRaw) => {
    let bodies = '';
    let categoryMatch;
    PLURAL_CATEGORY.lastIndex = 0;
    while ((categoryMatch = PLURAL_CATEGORY.exec(categoriesRaw)) !== null) {
      bodies += categoryMatch[2];
    }
    return bodies;
  });
}

/** Sorted, de-duplicated `{name}`/`${name}` placeholder tokens in a value. */
function placeholderTokens(value) {
  const stripped = stripPluralWrappers(value);
  const tokens = new Set();
  for (const match of stripped.matchAll(/\$\{\w+\}|\{\w+\}/g)) {
    tokens.add(match[0]);
  }
  return [...tokens].sort();
}

/**
 * Every token the engine must round-trip unchanged: placeholders plus the
 * protected brand/protocol words. A translation that drops one is rejected.
 */
function protectedTokens(value) {
  const stripped = stripPluralWrappers(value);
  const tokens = new Set(placeholderTokens(value));
  for (const brand of BRANDS) {
    if (new RegExp(`\\b${brand.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`).test(stripped)) {
      tokens.add(brand);
    }
  }
  return [...tokens].sort();
}

function hasSentinelResidue(value) {
  return value.includes(PUBLIC_USE_OPEN) || value.includes(PUBLIC_USE_CLOSE);
}

function sameTokens(a, b) {
  return JSON.stringify(protectedTokens(a)) === JSON.stringify(protectedTokens(b));
}

function countSentinels(text) {
  return (text.match(SENTINEL_RE) ?? []).length;
}

const sleep = ms => new Promise(r => setTimeout(r, ms));

async function callTranslate(googleCode, texts) {
  const body = texts.map(t => `q=${encodeURIComponent(t)}`).join('&');
  const response = await fetch(
    `https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=${googleCode}`,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        'User-Agent':
          'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Safari/537.36',
      },
      body,
    }
  );
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const json = await response.json();
  const list = Array.isArray(json) ? json : [json];
  if (list.length !== texts.length) {
    throw new Error(`batch length mismatch: sent ${texts.length}, got ${list.length}`);
  }
  return list.map(item => (typeof item === 'string' ? item : String(item)));
}

async function translateBatch(googleCode, texts, attempt = 0) {
  try {
    return await callTranslate(googleCode, texts);
  } catch (error) {
    if (attempt >= 5) {
      if (texts.length === 1) throw error;
      const mid = Math.ceil(texts.length / 2);
      const left = await translateBatch(googleCode, texts.slice(0, mid), 0);
      const right = await translateBatch(googleCode, texts.slice(mid), 0);
      return [...left, ...right];
    }
    await sleep(500 * 2 ** attempt);
    return translateBatch(googleCode, texts, attempt + 1);
  }
}

function chunkBySize(items, maxChars, maxItems) {
  const chunks = [];
  let current = [];
  let size = 0;
  for (const item of items) {
    const itemSize = item.text.length + 1;
    if (current.length > 0 && (size + itemSize > maxChars || current.length >= maxItems)) {
      chunks.push(current);
      current = [];
      size = 0;
    }
    current.push(item);
    size += itemSize;
  }
  if (current.length > 0) chunks.push(current);
  return chunks;
}

/**
 * Builds a per-key reconstruction plan. Text fragments that need translation
 * are pushed into `jobs`; the plan records how to reassemble each key.
 */
function planCatalog(enCatalog, existing, stores) {
  const jobs = [];
  const plans = [];

  for (const [key, rawValue] of Object.entries(enCatalog)) {
    if (typeof rawValue !== 'string') continue;
    if (existing[key] !== undefined) {
      // Only preserve hand-written/previous output when its protected tokens
      // are intact and it carries no leftover sentinel characters; a mangled
      // entry is re-translated instead of trusted.
      if (
        !hasSentinelResidue(existing[key]) &&
        existing[key].trim() !== '' &&
        sameTokens(existing[key], rawValue)
      ) {
        plans.push({ key, value: existing[key] });
        continue;
      }
    }
    if (rawValue === '') {
      plans.push({ key, value: '' });
      continue;
    }

    const plural = PLURAL_BLOCK.exec(rawValue);
    if (plural) {
      const before = rawValue.slice(0, plural.index);
      const after = rawValue.slice(plural.index + plural[0].length);
      const categories = [];
      let match;
      PLURAL_CATEGORY.lastIndex = 0;
      while ((match = PLURAL_CATEGORY.exec(plural[2])) !== null) {
        categories.push({ name: match[1], body: match[2] });
      }
      const store = [];
      const beforeJob = jobs.length;
      jobs.push({ text: protect(before, store), store });
      const categoryPlans = categories.map(category => {
        const jobIndex = jobs.length;
        jobs.push({ text: protect(category.body, store), store });
        return { name: category.name, jobIndex };
      });
      const afterJob = jobs.length;
      jobs.push({ text: protect(after, store), store });
      plans.push({
        key,
        store,
        plural: { countParam: plural[1], beforeJob, afterJob, categories: categoryPlans },
      });
      stores.push(store);
      continue;
    }

    const store = [];
    const jobIndex = jobs.length;
    jobs.push({ text: protect(rawValue, store), store });
    plans.push({ key, store, jobIndex });
    stores.push(store);
  }

  return { jobs, plans };
}

async function translateLocale(code) {
  const enCatalog = parseCatalogSource(readFileSync(resolve(i18nDir, 'en.ts'), 'utf8'));
  const fileName = `${code}.ts`;
  const existing = loadCatalog(fileName);
  const existingCount = Object.keys(existing).length;

  const stores = [];
  const { jobs, plans } = planCatalog(enCatalog, existing, stores);

  if (jobs.length === 0) {
    console.log(`[i18n] ${code}: already complete (${existingCount} preserved)`);
    return;
  }

  console.log(
    `[i18n] ${code}: ${Object.keys(enCatalog).length} keys, preserving ${existingCount}, translating ${jobs.length} fragments`
  );

  const chunks = chunkBySize(jobs, 1200, 40);
  let done = 0;
  for (const chunk of chunks) {
    const texts = chunk.map(job => job.text);
    let results;
    try {
      results = await translateBatch(GOOGLE_CODE[code], texts);
    } catch (error) {
      console.warn(`[i18n] ${code}: chunk failed (${error.message}); falling back to English for ${chunk.length} fragment(s)`);
      results = texts;
    }
    chunk.forEach((job, i) => {
      const result = results[i];
      // A faithful translation must round-trip every protected sentinel and
      // restore without leaving sentinel characters behind. If the engine
      // dropped or rewrote one, keep the English fragment verbatim instead of
      // emitting a value with broken interpolation.
      const restored = result ? restore(result, job.store) : '';
      const valid =
        Boolean(result) &&
        countSentinels(result) === countSentinels(job.text) &&
        !hasSentinelResidue(restored) &&
        (job.text.trim() === '' || restored.trim() !== '');
      job.translated = valid ? result : job.text;
    });
    done += chunk.length;
    if (done % 200 === 0 || done === jobs.length) {
      console.log(`[i18n] ${code}: ${done}/${jobs.length}`);
    }
    await sleep(120);
  }

  const out = {};
  for (const plan of plans) {
    if (plan.value !== undefined) {
      out[plan.key] = plan.value;
      continue;
    }
    if (plan.plural) {
      const before = restore(jobs[plan.plural.beforeJob].translated, plan.store);
      const after = restore(jobs[plan.plural.afterJob].translated, plan.store);
      const built = plan.plural.categories
        .map(category => {
          const body = restore(jobs[category.jobIndex].translated, plan.store);
          return `${category.name}{${body}}`;
        })
        .join(' ');
      out[plan.key] = `${before}{${plan.plural.countParam}, plural, ${built}}${after}`;
      continue;
    }
    out[plan.key] = restore(jobs[plan.jobIndex].translated, plan.store);
  }

  const residue = Object.entries(out).filter(([, value]) => hasSentinelResidue(value));
  if (residue.length > 0) {
    console.warn(`[i18n] ${code}: ${residue.length} value(s) still contain sentinel residue`);
    for (const [key, value] of residue.slice(0, 5)) {
      console.warn(`[i18n] ${code}:   ${key} -> ${JSON.stringify(value)}`);
    }
  }

  const lines = [];
  lines.push('/**');
  lines.push(` * ${ENGLISH_NAME[code]} (${code}) catalog.`);
  lines.push(' *');
  lines.push(' * Machine-translated baseline keyed off `en.ts`; hand-written entries');
  lines.push(' * are preserved verbatim when regenerating. Missing/omitted keys fall');
  lines.push(' * back to en-GB via `resolveKey` in `I18nContext.tsx`.');
  lines.push(' */');
  lines.push("import type { TranslationMap } from './types';");
  lines.push('');
  lines.push(`const ${camelVarName(code)}: TranslationMap = {`);
  for (const [key, value] of Object.entries(out)) {
    lines.push(`  ${JSON.stringify(key)}: ${JSON.stringify(value)},`);
  }
  lines.push('};');
  lines.push('');
  lines.push(`export default ${camelVarName(code)};`);
  lines.push('');

  mkdirSync(i18nDir, { recursive: true });
  writeFileSync(resolve(i18nDir, fileName), lines.join('\n'), 'utf8');
  console.log(`[i18n] ${code}: wrote ${Object.keys(out).length} keys -> ${fileName}`);
}

async function main() {
  const requested = process.argv.slice(2).filter(arg => !arg.startsWith('-'));
  const codes = requested.length > 0 ? requested : Object.keys(GOOGLE_CODE);
  for (const code of codes) {
    if (!GOOGLE_CODE[code]) {
      console.warn(`[i18n] skipping unknown locale: ${code}`);
      continue;
    }
    try {
      await translateLocale(code);
    } catch (error) {
      console.error(`[i18n] ${code}: failed — ${error.stack || error.message}`);
    }
  }
}

main();