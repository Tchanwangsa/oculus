import { PillTabs } from "@/components/ui/PillTabs";
import { SuggestToggle } from "./SuggestToggle";
import { cn } from "@/lib/utils";

export type EditorMode = "live" | "raw";

/** What the suggestions toggle shows: a request in flight, and the last
 *  failure until a request succeeds. */
export interface SuggestStatus {
  pending: boolean;
  error: string | null;
}

export const SUGGEST_IDLE: SuggestStatus = { pending: false, error: null };

/** The header's status word; `idle` is blank. */
export type SaveStatus =
  | { state: "idle" }
  | { state: "saving" }
  | { state: "saved" }
  | { state: "error"; message: string };

const MODES = [
  { value: "live", label: "Live" },
  { value: "raw", label: "Raw" },
] as const;

/** The save word, the AI-suggestions toggle and the Live/Raw pills, drawn in
 *  the host page's header. */
export function DocumentControls({
  mode,
  onMode,
  status,
  suggestions,
  onSuggestions,
  suggestStatus,
}: {
  mode: EditorMode;
  onMode: (mode: EditorMode) => void;
  status: SaveStatus;
  suggestions: boolean;
  onSuggestions: (on: boolean) => void;
  suggestStatus: SuggestStatus;
}) {
  const word =
    status.state === "saving" ? "Saving…"
    : status.state === "saved" ? "Saved"
    : status.state === "error" ? status.message
    : "";
  return (
    <div className="flex shrink-0 items-center gap-3">
      <span
        className={cn(
          "max-w-64 truncate text-[11px]",
          status.state === "error" ? "text-destructive" : "text-muted-foreground",
        )}
        title={status.state === "error" ? status.message : undefined}
      >
        {word}
      </span>
      <SuggestToggle on={suggestions} onChange={onSuggestions} status={suggestStatus} />
      <PillTabs tabs={MODES} value={mode} onChange={onMode} />
    </div>
  );
}
