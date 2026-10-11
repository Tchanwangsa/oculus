import { useRef, useState } from "react";

import { ModelPicker } from "@/components/harness/composer/ModelPicker";
import type { JobSelection } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";

/**
 * `ModelPicker` for one entry of a job's chain. Browsing another agent's tab
 * changes nothing: the tab is local state, back on the committed agent when
 * the menu closes without a pick, and only a model pick commits — so no entry
 * ever exists with an agent and no model.
 *
 * `onPick` fires once per gesture with the whole entry. The picker reports a
 * model pick as `onModel` then, in the same click, `onReasoning` with the
 * level the new model takes; both are staged and committed together on the
 * next microtask, so a caller that appends on `onPick` appends once.
 */
export function ChainPicker({
  providers,
  entry,
  fallbackProvider,
  onPick,
  onView,
  align = "start",
  emptyLabel,
  className,
}: {
  providers: PickerProvider[];
  /** The entry shown; null for an empty chain or an "add" picker. */
  entry: JobSelection | null;
  /** The tab the menu opens on when `entry` is null. */
  fallbackProvider: Provider;
  onPick: (pick: JobSelection) => void;
  /** Told each agent tab the student looks at, so its catalogue can load. */
  onView?: (provider: Provider) => void;
  align?: "start" | "end";
  emptyLabel?: string;
  className?: string;
}) {
  const committed = entry?.provider ?? fallbackProvider;
  const [viewed, setViewed] = useState<Provider | null>(null);
  const tab = viewed ?? committed;
  const onCommitted = tab === committed;

  const staged = useRef<JobSelection | null>(null);
  const stage = (patch: Partial<JobSelection>) => {
    const base: JobSelection = staged.current ?? {
      provider: tab,
      model: onCommitted ? (entry?.model ?? "") : "",
      reasoningEffort: onCommitted ? (entry?.reasoningEffort ?? null) : null,
    };
    const first = staged.current === null;
    staged.current = { ...base, ...patch };
    if (!first) return;
    queueMicrotask(() => {
      const pick = staged.current;
      staged.current = null;
      // A level toggled before any model is chosen has nothing to apply to.
      if (pick?.model) onPick(pick);
    });
  };

  const view = (p: Provider) => {
    setViewed(p);
    onView?.(p);
  };

  return (
    <ModelPicker
      providers={providers}
      provider={tab}
      providerLocked={false}
      model={onCommitted ? (entry?.model ?? null) : null}
      reasoning={onCommitted ? (entry?.reasoningEffort ?? null) : null}
      onProvider={view}
      onModel={(m) => {
        if (m) stage({ provider: tab, model: m });
      }}
      onReasoning={(level) => stage({ reasoningEffort: level })}
      onOpenChange={(open) => {
        if (open) onView?.(tab);
        else setViewed(null);
      }}
      align={align}
      emptyLabel={emptyLabel}
      className={className}
    />
  );
}
