import { useEffect, useState } from "react";
import { CircleNotch } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { displayCode } from "@/lib/format/format";
import { validThemeName } from "@/lib/studio/themeDraft";
import { writeTheme } from "@/lib/studio/themes";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * Saves a theme's text under a new name in the browsed library: Duplicate,
 * New theme, and saving an edit of the built-in theme, which cannot change.
 * The name must be a theme name that the library does not show yet.
 */
export function DuplicateDialog({
  open,
  title,
  suggested,
  taken,
  subject,
  text,
  onCancel,
  onSaved,
}: {
  open: boolean;
  title: string;
  suggested: string;
  /** The names already shown, which a copy would collide with. */
  taken: string[];
  /** The library's course code; null is the global library. */
  subject: string | null;
  /** The theme file to save, or null while it is being read. */
  text: string | null;
  onCancel: () => void;
  onSaved: (name: string) => void;
}) {
  const [name, setName] = useState(suggested);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (open) {
      setName(suggested);
      setError(null);
    }
  }, [open, suggested]);

  const problem = !validThemeName(name)
    ? "Lowercase letters, digits and -."
    : taken.includes(name)
      ? `There is already a theme called ${name}.`
      : null;
  const save = async () => {
    if (problem || text == null) return;
    setBusy(true);
    setError(null);
    try {
      await writeTheme(name, subject ? "subject" : "global", subject, text);
      onSaved(name);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && !busy && onCancel()}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            Saved in {subject ? `${displayCode(subject)}'s library` : "the global library"}, where any video can name it.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-1.5">
          <Input
            aria-label="Theme name"
            value={name}
            autoFocus
            spellCheck={false}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
            }}
          />
          {(problem || error) && (
            <p data-selectable className="text-[12px] leading-snug text-destructive">
              {error ?? problem}
            </p>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={onCancel}>
            Cancel
          </Button>
          <Button disabled={busy || problem != null || text == null} onClick={() => void save()}>
            {busy && <CircleNotch className="animate-spin" aria-hidden />}
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
