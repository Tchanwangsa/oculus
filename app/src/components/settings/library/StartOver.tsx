import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowCounterClockwise } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { useSyncStore } from "@/stores/sync/syncStore";
import { Section } from "@/components/settings/shared/section";

/** Browser-side state that names the old library's subjects, files, threads
 *  or tabs. Display preferences (theme, zoom, panel widths) stay. */
const LIBRARY_KEYS = [
  "oculus-tabs",
  "oculus-recent-tabs",
  "oculus-recents",
  "oculus-last-subject",
  "oculus-subject-icons",
  "oculus-math-subject-recents",
  "oculus-lecture-progress",
  "oculus-document-history",
  "oculus-chat-groups-collapsed",
  "oculus-chat-groups-order",
  "oculus-projects-groups-collapsed",
  "oculus.chatDrafts",
  "oculus.chatPastes",
];

/**
 * Empties the library so Oculus can be set up for another university: Rust
 * signs out, forgets the university's credentials and restarts the app, which
 * deletes the data before anything opens it (`library/storage`).
 */
export function StartOver() {
  const scraping = useSyncStore((s) => s.scraping);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const startOver = async () => {
    setBusy(true);
    setError(null);
    try {
      for (const key of LIBRARY_KEYS) {
        try {
          localStorage.removeItem(key);
        } catch {
          /* A blocked store has nothing to clear. */
        }
      }
      await invoke("storage_start_over");
    } catch (e) {
      setError(String(e));
      setBusy(false);
      setConfirming(false);
    }
  };

  return (
    <Section
      title="Start over"
      description="Clear the library and sign out, to set Oculus up again — for another university, say."
    >
      <div className="flex items-center justify-between gap-4 py-2">
        <p className="text-xs text-muted-foreground">
          Your API keys and cloud usage limits are kept.
        </p>
        <Button
          variant="outline"
          size="xs"
          className="shrink-0 text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
          disabled={scraping}
          title={scraping ? "Cancel the sync first" : undefined}
          onClick={() => setConfirming(true)}
        >
          <ArrowCounterClockwise size={13} /> Start over…
        </Button>
      </div>
      {error && <p className="text-xs text-destructive py-1">{error}</p>}
      <ConfirmDialog
        open={confirming}
        onCancel={() => setConfirming(false)}
        onConfirm={startOver}
        title="Start over?"
        description="This deletes every synced file and lecture, your notes, uploads, projects and chats, signs you out and forgets your Canvas address. Oculus then restarts empty. It can't be undone."
        confirmLabel="Delete and restart"
        busyLabel="Restarting…"
        busy={busy}
      />
    </Section>
  );
}
