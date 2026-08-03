#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const tempHome = mkdtempSync(join(tmpdir(), "minutes-proactive-context-test-"));

const modulePath = new URL("../lib/proactive-context.mjs", import.meta.url);
const { buildProactiveContextBundle, proactiveContextAdditionalText } = await import(modulePath.href);

try {
  const memosDir = join(tempHome, "meetings", "memos");
  mkdirSync(memosDir, { recursive: true });
  writeFileSync(
    join(memosDir, "2026-04-09-pricing.md"),
    "---\ntitle: Pricing Idea\ndate: 2026-04-09T12:00:00-07:00\n---\n",
  );

  const bundle = await buildProactiveContextBundle(tempHome);
  assert.equal(bundle.memos.length, 1);
  assert.equal(bundle.staleCommitments.length, 0);
  assert.equal(bundle.losingTouch.length, 0);

  const text = proactiveContextAdditionalText({
    memos: ["[Apr 9] Pricing Idea"],
    staleCommitments: ['"Send follow-up" for Alex'],
    losingTouch: ["Sarah (last 9d ago)"],
  });

  assert.match(text, /Recent voice memos:/);
  assert.match(text, /Stale commitments/);
  assert.match(text, /Losing touch:/);

  console.log("proactive-context tests passed");
} finally {
  rmSync(tempHome, { recursive: true, force: true });
}
