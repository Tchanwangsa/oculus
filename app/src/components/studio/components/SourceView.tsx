import { CodeText } from "@/components/markdown/MdComponents";
import { Skeleton } from "@/components/ui/skeleton";
import type { SourceState } from "@/hooks/studio/useComponentSource";
import { displayCode } from "@/lib/format/format";

/** The component's TSX, read-only, from the level that wins. Changes go
 *  through the agent ("Ask for a change"), not through this view. */
export function SourceView({ state, subject }: { state: SourceState; subject: string | null }) {
  if (state.error) {
    return (
      <p data-selectable className="text-[12.5px] leading-snug text-destructive">
        {state.error}
      </p>
    );
  }
  if (!state.source) return <Skeleton className="h-40 w-full rounded-lg" />;
  const { level, text } = state.source;
  return (
    <div className="space-y-1.5">
      <p className="text-[11.5px] text-muted-foreground">
        {level === "subject" && subject ? `From ${displayCode(subject)}'s own library` : "From the global library"}
      </p>
      <CodeText className="max-h-96 overflow-auto whitespace-pre break-normal rounded-lg bg-secondary px-3 py-2.5">
        {text}
      </CodeText>
    </div>
  );
}
