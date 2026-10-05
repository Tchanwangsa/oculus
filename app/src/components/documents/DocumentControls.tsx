import { useEffect, useRef, useState } from "react";
import { BookmarkSimple, CircleNotch, ClockCounterClockwise } from "@phosphor-icons/react";

import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { PillTabs } from "@/components/ui/PillTabs";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { DocumentVersion } from "@/lib/documentVersions";
import { cn } from "@/lib/utils";
import { useActivePaneId } from "@/stores/tabStore";
import { SuggestToggle } from "./SuggestToggle";

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

/** What the header asks of the mounted editor (`DocumentEditor`'s `actions`). */
export interface DocumentActions {
  /** Flush, then save the editor's text as a checkpoint. */
  saveVersion(label?: string): Promise<DocumentVersion>;
}

const MODES = [
  { value: "live", label: "Live" },
  { value: "raw", label: "Raw" },
] as const;

/** How long "Saved v3" stands in for the save word. */
const SAVED_VERSION_MS = 2500;

/** Keeps the note focused, so toggling the panel leaves the caret where it was. */
const keepFocus = (e: React.MouseEvent) => e.preventDefault();

/** The save word, Save version, the history toggle, the AI-suggestions toggle
 *  and the Live/Raw pills, drawn in the host page's header. */
export function DocumentControls({
  mode,
  onMode,
  status,
  suggestions,
  onSuggestions,
  suggestStatus,
  history,
  onHistory,
  onSaveVersion,
}: {
  mode: EditorMode;
  onMode: (mode: EditorMode) => void;
  status: SaveStatus;
  suggestions: boolean;
  onSuggestions: (on: boolean) => void;
  suggestStatus: SuggestStatus;
  history: boolean;
  onHistory: (open: boolean) => void;
  onSaveVersion: (label?: string) => Promise<DocumentVersion>;
}) {
  // A saved version briefly takes the save word's place: the feedback, with
  // no toast. A save error still wins.
  const [savedVersion, setSavedVersion] = useState<string | null>(null);
  useEffect(() => {
    if (!savedVersion) return;
    const t = setTimeout(() => setSavedVersion(null), SAVED_VERSION_MS);
    return () => clearTimeout(t);
  }, [savedVersion]);

  const word =
    status.state === "error" ? status.message
    : savedVersion ? savedVersion
    : status.state === "saving" ? "Saving…"
    : status.state === "saved" ? "Saved"
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
      <div className="flex items-center gap-1">
        <SaveVersionButton
          onSave={onSaveVersion}
          onSaved={(v) => setSavedVersion(v.number != null ? `Saved v${v.number}` : "Saved version")}
        />
        <Button
          variant="ghost"
          size="icon-xs"
          aria-pressed={history}
          aria-label="Version history"
          title="Version history"
          onMouseDown={keepFocus}
          onClick={() => onHistory(!history)}
          className={cn(
            history ? "text-brand hover:text-brand" : "text-muted-foreground hover:text-foreground",
          )}
        >
          <ClockCounterClockwise size={14} weight={history ? "bold" : "regular"} />
        </Button>
        <SuggestToggle on={suggestions} onChange={onSuggestions} status={suggestStatus} />
      </div>
      <PillTabs tabs={MODES} value={mode} onChange={onMode} />
    </div>
  );
}

/**
 * "Save version": a popover asking for an optional name, Enter or Save to
 * keep it. ⇧⌘S opens it and, open, saves — in the focused pane only, so a
 * page and its side panel showing two notes open one. ⌘S stays a plain
 * flush (the editor's).
 * A failure stays in the popover; closing it hands focus back to wherever it
 * was, usually the note.
 */
function SaveVersionButton({
  onSave,
  onSaved,
}: {
  onSave: (label?: string) => Promise<DocumentVersion>;
  onSaved: (version: DocumentVersion) => void;
}) {
  const [open, setOpen] = useState(false);
  const [label, setLabel] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Focused before the popover took it. */
  const returnTo = useRef<HTMLElement | null>(null);

  const show = (next: boolean) => {
    if (next) {
      returnTo.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setLabel("");
      setError(null);
    }
    setOpen(next);
  };

  const save = async () => {
    if (saving) return;
    setSaving(true);
    setError(null);
    try {
      const version = await onSave(label.trim() || undefined);
      setOpen(false);
      onSaved(version);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };
  // The key listener outlives renders; it reads the newest closures here.
  const latest = useRef({ open, show, save });
  latest.current = { open, show, save };

  const tabActive = useTabActive();
  const paneId = useTabId();
  const focused = useActivePaneId() === paneId;
  useEffect(() => {
    if (!tabActive || !focused) return;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey || !e.shiftKey) return;
      if (e.key.toLowerCase() !== "s") return;
      e.preventDefault();
      if (latest.current.open) void latest.current.save();
      else latest.current.show(true);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [tabActive, focused]);

  return (
    <Popover open={open} onOpenChange={show}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="xs"
          title="Save version (⇧⌘S)"
          className="text-muted-foreground hover:text-foreground"
        >
          <BookmarkSimple size={13} />
          Save version
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="w-72 p-3"
        onCloseAutoFocus={(e) => {
          const el = returnTo.current;
          returnTo.current = null;
          if (el?.isConnected) {
            e.preventDefault();
            el.focus();
          }
        }}
      >
        <Input
          autoFocus
          value={label}
          placeholder="Name this version (optional)"
          aria-label="Version name"
          onChange={(e) => setLabel(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void save();
            }
          }}
          className="h-8 rounded-lg text-[13px]"
        />
        {error && <p className="mt-2 text-[11px] break-words text-destructive">{error}</p>}
        <Button size="sm" disabled={saving} onClick={() => void save()} className="mt-3 w-full">
          {saving && <CircleNotch className="animate-spin" aria-hidden />}
          Save version
        </Button>
      </PopoverContent>
    </Popover>
  );
}
