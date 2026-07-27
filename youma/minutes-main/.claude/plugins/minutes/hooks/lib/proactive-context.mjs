#!/usr/bin/env node

import { existsSync, readFileSync } from "fs";
import { join } from "path";
import { homedir } from "os";

export async function buildProactiveContextBundle(baseDir = homedir()) {
  const memos = [];
  const staleCommitments = [];
  const losingTouch = [];

  try {
    const memosDir = join(baseDir, "meetings", "memos");
    if (existsSync(memosDir)) {
      const { readdirSync, statSync } = await import("fs");
      const cutoff = Date.now() - 3 * 24 * 60 * 60 * 1000;
      for (const name of readdirSync(memosDir)) {
        if (!name.endsWith(".md")) continue;
        const full = join(memosDir, name);
        const mtime = statSync(full).mtimeMs;
        if (mtime < cutoff) continue;
        try {
          const content = readFileSync(full, "utf-8");
          const titleMatch = content.match(/^title:\s*(.+)$/m);
          const dateMatch = content.match(/^date:\s*(.+)$/m);
          const title = titleMatch ? titleMatch[1].trim() : name.replace(".md", "");
          const date = dateMatch
            ? new Date(dateMatch[1].trim()).toLocaleDateString("en-US", { month: "short", day: "numeric" })
            : "recent";
          memos.push(`[${date}] ${title}`);
        } catch {
          memos.push(name.replace(".md", ""));
        }
      }
    }
  } catch {
    // Non-fatal.
  }

  try {
    const { execFileSync } = await import("child_process");
    const minutesBin = join(baseDir, ".local", "bin", "minutes");
    if (existsSync(minutesBin)) {
      const peopleRaw = execFileSync(minutesBin, ["people", "--json", "--limit", "10"], {
        encoding: "utf-8",
        timeout: 3000,
      });
      const people = JSON.parse(peopleRaw);
      if (Array.isArray(people)) {
        for (const person of people.filter((p) => p.losing_touch).slice(0, 3)) {
          losingTouch.push(
            `${person.name} (${person.meeting_count} meetings, last ${Math.round(person.days_since)}d ago)`,
          );
        }
      }

      try {
        const commitsRaw = execFileSync(minutesBin, ["commitments", "--json"], {
          encoding: "utf-8",
          timeout: 3000,
        });
        const commitments = JSON.parse(commitsRaw);
        if (Array.isArray(commitments)) {
          for (const item of commitments.filter((c) => c.status === "stale").slice(0, 3)) {
            staleCommitments.push(`"${item.text}" for ${item.person_name || "unknown"}`);
          }
        }
      } catch {
        // Non-fatal.
      }
    }
  } catch {
    // Non-fatal.
  }

  return {
    memos,
    staleCommitments,
    losingTouch,
    summary: `${memos.length} memos · ${staleCommitments.length} stale commitments · ${losingTouch.length} losing-touch alerts`,
  };
}

export function proactiveContextAdditionalText(bundle) {
  let text = "";
  if (bundle.memos.length > 0) {
    text += `\n\nRecent voice memos: ${bundle.memos.join(", ")}. The user may ask about these — use search_meetings or get_meeting MCP tools to retrieve details.`;
  }
  if (bundle.losingTouch.length > 0) {
    text += `\n\nLosing touch: ${bundle.losingTouch.join(", ")}. Consider reaching out.`;
  }
  if (bundle.staleCommitments.length > 0) {
    text += `\n\nStale commitments (overdue): ${bundle.staleCommitments.join("; ")}. Mention if relevant to today's work.`;
  }
  return text;
}
