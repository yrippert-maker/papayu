/**
 * B3: Mapper/validation for applyable proposals.
 * UI applies only what is recognized by whitelist (e.g. onlineAutoUseAsContext).
 */

import type { WeeklyProposal } from "@/lib/types";

/** Stable key for a proposal (for applied set / dedup). */
export function proposalKey(p: WeeklyProposal): string {
  const raw = `${p.kind}:${p.title}`;
  let h = 0;
  for (let i = 0; i < raw.length; i++) {
    h = (h << 5) - h + raw.charCodeAt(i);
    h |= 0;
  }
  return `proposal_${h >>> 0}`;
}

const ONLINE_AUTO_PATTERNS = [
  /onlineAutoUseAsContext/i,
  /online\s*auto\s*use\s*as\s*context/i,
  /auto[- ]?use\s*online\s*context/i,
  /enable\s*online\s*context/i,
];

function textContainsOnlineAuto(text: string): boolean {
  return ONLINE_AUTO_PATTERNS.some((re) => re.test(text));
}

/** MVP: only onlineAutoUseAsContext. Returns key/value when title or steps clearly refer to it. */
export function extractSettingChange(
  p: WeeklyProposal
): { key: "onlineAutoUseAsContext"; value: boolean } | null {
  if (p.kind !== "setting_change") return null;
  const steps = p.steps ?? [];
  const title = (p.title ?? "").trim();
  const evidence = (p.evidence ?? "").trim();
  const combined = [title, ...steps, evidence].join(" ");
  if (!textContainsOnlineAuto(combined)) return null;
  const lower = combined.toLowerCase();
  if (/\b(disable|turn\s*off|false|off)\b/.test(lower)) {
    return { key: "onlineAutoUseAsContext", value: false };
  }
  if (/\b(enable|turn\s*on|true|on)\b/.test(lower)) {
    return { key: "onlineAutoUseAsContext", value: true };
  }
  return { key: "onlineAutoUseAsContext", value: true };
}
