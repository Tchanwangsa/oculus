import { describe, expect, test } from "bun:test";
import type { JobSelection } from "@/lib/db";
import type { PickerProvider } from "@/lib/harness";
import { providersFor, withEntry } from "@/components/settings/jobs/chain";

const pick = (provider: JobSelection["provider"], model: string): JobSelection => ({
  provider,
  model,
  reasoningEffort: null,
});
const models = (chain: JobSelection[]) => chain.map((e) => e.model);

describe("a job's chain holds one model per agent", () => {
  test("a second model of an agent replaces the first where it is picked", () => {
    const chain = [pick("claude", "haiku"), pick("codex", "luna")];
    expect(models(withEntry(chain, 0, pick("claude", "sonnet")))).toEqual(["sonnet", "luna"]);
    expect(models(withEntry(chain, 2, pick("claude", "sonnet")))).toEqual(["luna", "sonnet"]);
  });

  test("opencode takes several models, but never the same one twice", () => {
    const chain = [pick("opencode", "a"), pick("claude", "haiku")];
    expect(models(withEntry(chain, 2, pick("opencode", "b")))).toEqual(["a", "haiku", "b"]);
    expect(models(withEntry(chain, 2, pick("opencode", "a")))).toEqual(["haiku", "a"]);
  });

  test("an entry may take any agent no other entry holds, and opencode always", () => {
    const all = (["claude", "codex", "opencode"] as const).map(
      (id) => ({ id, label: id, models: [] }) as unknown as PickerProvider,
    );
    const chain = [pick("claude", "haiku"), pick("opencode", "a")];
    expect(providersFor(all, chain).map((p) => p.id)).toEqual(["codex", "opencode"]);
    expect(providersFor(all, chain, 0).map((p) => p.id)).toEqual(["claude", "codex", "opencode"]);
  });
});
