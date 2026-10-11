import { invoke } from "@tauri-apps/api/core";

/** Store the rule and drop the thread's `agy` process (it never re-reads its
 *  rules); the caller's follow-up resumes under them. Resolves to all rules. */
export function harnessAntigravityAllow(threadId: number, rule: string): Promise<string[]> {
  return invoke<string[]>("harness_antigravity_allow", { threadId, rule });
}

export function harnessAntigravityRules(): Promise<string[]> {
  return invoke<string[]>("harness_antigravity_rules");
}

export function harnessAntigravityRevoke(rule: string): Promise<string[]> {
  return invoke<string[]>("harness_antigravity_revoke", { rule });
}
