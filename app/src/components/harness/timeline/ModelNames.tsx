import { createContext, memo, useContext, useMemo, type ReactNode } from "react";
import { ArrowsLeftRight, ChatsCircle } from "@phosphor-icons/react";
import { useProviderModels } from "@/hooks/agents/useProviderModels";
import { runDivider, type ModelName, type Provider } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import type { RunChange } from "./rows";
import { SystemDivider } from "./SystemDivider";

const ModelNames = createContext<ModelName>((_provider, model) => model);

/** The model-name lookup footers and dividers read: the catalogues of the
 *  agents this thread's runs went to, read without starting a CLI that only
 *  its Settings page starts (`start: false`). A model no catalogue lists is
 *  named by its id. Children pass through unchanged, so a catalogue landing
 *  redraws only the rows that read a name. */
export function ModelNamesProvider({ threadId, children }: { threadId: number | null; children: ReactNode }) {
  // A primitive, so a run's settling does not redraw the provider.
  const key = useHarnessStore((s) => {
    const runs = threadId == null ? undefined : s.runs[threadId];
    return [...new Set(Object.values(runs ?? {}).flatMap((r) => (r.provider ? [r.provider] : [])))].sort().join(" ");
  });
  const needed = useMemo(() => (key ? (key.split(" ") as Provider[]) : []), [key]);
  const { modelsFor } = useProviderModels(needed, { start: false });
  const name = useMemo<ModelName>(
    () => (provider, model) => modelsFor(provider).find((m) => m.id === model)?.label ?? model,
    [modelsFor],
  );
  return <ModelNames.Provider value={name}>{children}</ModelNames.Provider>;
}

export const useModelName = () => useContext(ModelNames);

/** Above a run's question: `{from} → {to}` where it switched agent, model or
 *  level, and its context handoff ("Context handoff", "… pending", danger
 *  tone when it was never delivered), in one line. */
export const RunDividerRow = memo(function RunDividerRow({ change }: { change: RunChange }) {
  const name = useModelName();
  const divider = runDivider(change.before, change.run, name);
  if (!divider) return null;
  return (
    <div title={divider.title} className="pb-1">
      <SystemDivider
        icon={divider.switched ? ArrowsLeftRight : ChatsCircle}
        label={divider.label}
        detail={divider.detail}
        tone={divider.tone}
      />
    </div>
  );
}, (a, b) => a.change.run === b.change.run && a.change.before === b.change.before);
