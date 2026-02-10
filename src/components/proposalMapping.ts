/**
 * B3: Safe mapping layer — re-exports from lib/proposals and adapts to ApplyableAction.
 */

import type { WeeklyProposal } from "@/lib/types";
import { extractSettingChange } from "@/lib/proposals";

export type ApplyableSetting = "onlineAutoUseAsContext";

export interface ApplyableAction {
  kind: "setting";
  key: ApplyableSetting;
  value: boolean;
}

/** Returns an applyable action only when the proposal maps to a whitelisted setting. */
export function canApplyProposal(p: WeeklyProposal): ApplyableAction | null {
  const change = extractSettingChange(p);
  if (!change) return null;
  return { kind: "setting", key: change.key, value: change.value };
}
