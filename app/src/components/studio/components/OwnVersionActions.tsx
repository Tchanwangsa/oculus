import { useState } from "react";
import { CopySimple, Trash } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { displayCode } from "@/lib/format/format";
import type { LibraryItemKind } from "@/lib/studio/changeRequest";
import { componentSource, deleteComponent, writeComponent } from "@/lib/studio/components";
import { deleteTheme, themeSource, writeTheme } from "@/lib/studio/themes";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** The global text copied into the subject's library, unchanged. */
async function makeOwn(kind: LibraryItemKind, name: string, subject: string) {
  if (kind === "component") {
    const { text } = await componentSource(name, null);
    await writeComponent(name, "subject", subject, text);
  } else {
    const { text } = await themeSource(name, null);
    await writeTheme(name, "subject", subject, text);
  }
}

async function removeOwn(kind: LibraryItemKind, name: string, subject: string) {
  if (kind === "component") await deleteComponent(name, subject);
  else await deleteTheme(name, "subject", subject);
}

/**
 * In a subject's library: "Own version" copies a global component or theme
 * into the subject's library so it can change there without touching other
 * subjects, and "Remove own version" deletes that copy so the subject uses
 * the global one again (the copy is kept in the library's history).
 */
export function OwnVersionActions({
  kind,
  name,
  level,
  shadows,
  subject,
  onDone,
}: {
  kind: LibraryItemKind;
  name: string;
  /** Where the item the subject sees comes from. */
  level: "bundled" | "global" | "subject";
  /** The subject's file replaces a global one. */
  shadows: boolean;
  /** The library's course code; nothing shows in the global library. */
  subject: string | null;
  onDone: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!subject) return null;
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      setConfirm(false);
      onDone();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };
  const code = displayCode(subject);
  const noun = kind === "component" ? "component" : "theme";

  return (
    <>
      {level === "global" && (
        <Button
          type="button"
          variant="outline"
          size="xs"
          disabled={busy}
          title={`Copy the global ${noun} into ${code}'s library to change it there only`}
          onClick={() => void run(() => makeOwn(kind, name, subject))}
        >
          <CopySimple size={12} />
          Own version
        </Button>
      )}
      {level === "subject" && shadows && (
        <Button
          type="button"
          variant="ghost"
          size="xs"
          disabled={busy}
          title={`Delete ${code}'s copy; ${code} uses the global ${noun} again`}
          onClick={() => setConfirm(true)}
        >
          <Trash size={12} />
          Remove own version
        </Button>
      )}
      {error && (
        <p data-selectable className="basis-full text-[12px] leading-snug text-destructive">
          {error}
        </p>
      )}
      <ConfirmDialog
        open={confirm}
        onCancel={() => setConfirm(false)}
        onConfirm={() => void run(() => removeOwn(kind, name, subject))}
        title={`Remove ${code}'s own ${name}?`}
        description={`${code} goes back to the global ${noun}. Its own version stays in the library's history.`}
        confirmLabel="Remove"
        busyLabel="Removing"
        busy={busy}
      />
    </>
  );
}
