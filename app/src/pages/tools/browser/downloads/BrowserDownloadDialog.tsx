import { useEffect, useState } from "react";
import { CircleNotch, Warning } from "@phosphor-icons/react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { DriveTree } from "@/components/drive";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useSubjects } from "@/hooks/data/useSubjects";
import { browser } from "@/lib/browser";
import { downloads } from "@/lib/browser/downloads/commands";
import { sizeWords } from "@/lib/browser/downloads/describe";
import { guessSubject, preselectSubject } from "@/lib/browser/downloads/subjectGuess";
import type { Download, LastDestination } from "@/lib/browser/downloads/types";
import { isPipelineFile } from "@/lib/files/fileTypes";
import { subjectRel } from "@/lib/files/libraryLayout";
import { baseName } from "@/lib/drive/paths";
import { displayCode } from "@/lib/format/format";
import { isWindows } from "@/lib/platform";
import { useAskedDownload, useBrowserDownloadsStore } from "@/stores/shell/browserDownloadsStore";
import { useBrowserPrefsStore } from "@/stores/shell/browserPrefsStore";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { useInstitution } from "@/stores/sync/institutionStore";
import { DestinationOption } from "./DestinationOption";
import { DownloadSubjectSelect } from "./DownloadSubjectSelect";
import { problem, THIS_COMPUTER } from "./words";

/**
 * The Save dialog: where a download goes, asked while its bytes still arrive.
 * A portal, so it draws over the browser page and the holes keep its clicks
 * (`nativeHoles.ts`); Radix focuses its first control on open, which takes
 * the keyboard from the page on macOS, and on Windows Rust hands it over
 * (`browser.focusApp`). Dismissing it never discards: the download waits in
 * the toolbar's list. Mounted once, in AppLayout, so it also opens over a
 * page that is not the browser's.
 */
export function BrowserDownloadDialog() {
  const download = useAskedDownload();
  const dismiss = useBrowserDownloadsStore((s) => s.dismiss);
  const [busy, setBusy] = useState(false);
  const asked = download?.id;
  useEffect(() => {
    if (asked != null && isWindows) browser.focusApp().catch(() => {});
  }, [asked]);
  // A choice's own event may have closed it already; never close another.
  const done = (id: number) => {
    if (useBrowserDownloadsStore.getState().prompt.open === id) dismiss();
  };
  return (
    <Dialog open={!!download} onOpenChange={(open) => !open && !busy && dismiss()}>
      {download ? (
        <SaveForm
          key={download.id}
          download={download}
          busy={busy}
          setBusy={setBusy}
          onDone={() => done(download.id)}
        />
      ) : null}
    </Dialog>
  );
}

function SaveForm({
  download,
  busy,
  setBusy,
  onDone,
}: {
  download: Download;
  busy: boolean;
  setBusy: (busy: boolean) => void;
  onDone: () => void;
}) {
  const { subjects } = useSubjects();
  const institution = useInstitution();
  // A download-only tab closes as the dialog opens: the address it was
  // opened at is kept, since it still names the subject.
  const liveUrl = useBrowserStore((s) => s.tabs.find((t) => t.id === download.tab)?.url);
  const [tabUrl, setTabUrl] = useState(liveUrl);
  if (liveUrl && liveUrl !== tabUrl) setTabUrl(liveUrl);
  const lastDestination = useBrowserPrefsStore((s) => s.lastDestination);
  const lastSubject = useBrowserPrefsStore((s) => s.lastSubject);
  const remember = useBrowserPrefsStore((s) => s.rememberDestination);

  // Preselections stay derived until the student picks, since the subjects
  // and the profile load after the dialog opens.
  const [pickedDest, setPickedDest] = useState<LastDestination | null>(null);
  const [pickedSubject, setPickedSubject] = useState<string | null>(null);
  // A folder in the chosen subject, data-dir-relative; the subject's own
  // root until the student picks another.
  const [pickedFolder, setPickedFolder] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const dest = pickedDest ?? lastDestination;
  const subject =
    pickedSubject ??
    preselectSubject(guessSubject(tabUrl, subjects, institution?.canvas ?? null), lastSubject, subjects);
  const root = subject ? subjectRel(subject) : null;
  const folder = root && pickedFolder?.startsWith(`${root}/`) ? pickedFolder : root;

  const run = async (action: () => Promise<boolean>) => {
    setBusy(true);
    setError(null);
    try {
      if (await action()) onDone();
    } catch (e) {
      setError(problem(e));
    } finally {
      setBusy(false);
    }
  };

  const save = () =>
    run(async () => {
      if (dest === "oculus") {
        if (!subject) return false;
        await downloads.choose(download.id, {
          kind: "oculus",
          subject_code: subject,
          ...(folder && folder !== root ? { target_dir: folder } : {}),
        });
        void remember("oculus", subject).catch(() => {});
        return true;
      }
      // A cancelled panel leaves the dialog as it was.
      const path = await downloads.savePanel(download.id);
      if (!path) return false;
      await downloads.choose(download.id, { kind: "path", path });
      void remember("mac").catch(() => {});
      return true;
    });

  const discard = () =>
    run(async () => {
      await downloads.choose(download.id, { kind: "discard" });
      return true;
    });

  const shown = error ?? (download.error ? `Not saved: ${download.error}` : null);
  const place = folder && folder !== root ? baseName(folder) : subject ? displayCode(subject) : "the subject";
  const kept = isPipelineFile(download.name)
    ? `Lands in ${place}, read and made searchable`
    : `Lands in ${place}, kept but not searchable`;

  return (
    <DialogContent className="sm:max-w-md" showCloseButton={false}>
      <DialogHeader className="gap-1">
        <div className="flex items-baseline justify-between gap-3">
          <DialogTitle className="min-w-0 break-words">Save “{download.name}”</DialogTitle>
          <span className="shrink-0 text-[12px] text-muted-foreground tabular-nums">
            {sizeWords(download)}
          </span>
        </div>
        <DialogDescription>
          {download.host ? `From ${download.host}` : "From this page"}
        </DialogDescription>
      </DialogHeader>

      <div role="radiogroup" aria-label="Where to save it" className="grid gap-2">
        <DestinationOption
          checked={dest === "oculus"}
          onSelect={() => setPickedDest("oculus")}
          title="In Oculus"
          hint={kept}
          disabled={busy}
          control={
            <DownloadSubjectSelect
              subjects={subjects}
              value={subject}
              disabled={busy}
              onChange={(code) => {
                setPickedSubject(code);
                setPickedFolder(null);
                setPickedDest("oculus");
              }}
              className="max-w-40 shrink-0"
            />
          }
        />
        <DestinationOption
          checked={dest === "mac"}
          onSelect={() => setPickedDest("mac")}
          title={`On this ${THIS_COMPUTER}…`}
          hint="Choose the folder in the save panel"
          disabled={busy}
        />
      </div>

      {dest === "oculus" && root ? (
        // The subject's folders; a synced folder can't be picked.
        <div className="max-h-48 overflow-y-auto rounded-lg border border-border-subtle p-1" aria-label="Folder">
          <DriveTree
            key={root}
            root={root}
            current={folder}
            onPick={(rel) => {
              setPickedFolder(rel);
              setPickedDest("oculus");
            }}
            storageKey="oculus-download-folder-open"
          />
        </div>
      ) : null}

      {shown ? (
        <Alert variant="warning" className="w-auto px-2.5 py-2">
          <Warning aria-hidden />
          <AlertDescription className="text-[11px] leading-snug">{shown}</AlertDescription>
        </Alert>
      ) : null}

      <DialogFooter>
        <Button variant="outline" disabled={busy} onClick={() => void discard()}>
          Discard
        </Button>
        <Button disabled={busy || (dest === "oculus" && !subject)} onClick={() => void save()}>
          {busy ? <CircleNotch className="animate-spin" aria-hidden /> : null}
          {dest === "mac" ? "Save…" : "Save"}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
