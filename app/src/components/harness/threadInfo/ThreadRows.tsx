import { useMemo } from "react";
import { Books } from "@phosphor-icons/react";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { usageTint } from "@/components/harness/composer/UsageMeter";
import { useProviderModels } from "@/hooks/agents/useProviderModels";
import type { Subject } from "@/lib/db";
import { displayCode } from "@/lib/format/format";
import { providerLabel, reasoningLabel, type HarnessThread, type Provider, type ThreadUsage } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { InfoRow } from "./InfoRow";
import { openTarget } from "./openTarget";

/** The thread's own facts: its subject (opening the subject beside the
 *  chat), the model and reasoning its next message uses, and how full its
 *  context window is, from the numbers the composer's usage ring reads. */
export function ThreadRows({
  thread,
  subjects,
  provider,
  model,
  effort,
  usage,
  onOpened,
}: {
  thread: HarnessThread;
  subjects: Subject[];
  provider: Provider;
  model: string | null;
  effort: string | null;
  usage: ThreadUsage | null;
  onOpened?: () => void;
}) {
  const subject = thread.subject_id == null ? null : subjects.find((s) => s.id === thread.subject_id);
  // The catalogue as already read; no CLI is started for a name.
  const { modelsFor } = useProviderModels(provider, { start: false });
  const modelName = useMemo(
    () => (model ? (modelsFor(provider).find((m) => m.id === model)?.label ?? model) : null),
    [model, modelsFor, provider],
  );
  const used = usage?.contextTokens ?? null;
  const limit = usage?.contextWindow ?? null;
  const percent = used != null && limit ? Math.max(0, Math.min(100, (used / limit) * 100)) : null;

  return (
    <>
      {subject ? (
        <InfoRow
          icon={<SubjectIcon code={subject.code} size={15} />}
          label={
            <>
              {displayCode(subject.code)}
              <span className="text-muted-foreground"> · {subject.name}</span>
            </>
          }
          title={subject.name}
          onOpen={(newTab) => {
            openTarget({ kind: "route", href: `/subjects/${subject.id}` }, newTab);
            onOpened?.();
          }}
        />
      ) : (
        <InfoRow icon={<Books size={15} className="text-muted-foreground" />} label="General" title="The whole library" />
      )}
      <InfoRow
        icon={<ProviderMark provider={provider} className="size-[15px]" />}
        label={
          <>
            {modelName ?? providerLabel(provider)}
            {effort && <span className="text-muted-foreground"> {reasoningLabel(effort)}</span>}
          </>
        }
        trailing={percent != null ? `${Math.round(percent)}% context` : undefined}
      />
      {percent != null && (
        <div
          role="meter"
          aria-label="Context window used"
          aria-valuenow={Math.round(percent)}
          aria-valuemin={0}
          aria-valuemax={100}
          className="mx-2.5 mt-0.5 mb-1.5 h-1 overflow-hidden rounded-full bg-secondary"
        >
          <div className={cn("h-full rounded-full", usageTint(percent))} style={{ width: `${percent}%` }} />
        </div>
      )}
    </>
  );
}
