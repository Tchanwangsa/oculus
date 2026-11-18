import type { ReactNode } from "react";
import { CircleNotch, FolderSimpleDashed, FolderSimplePlus, Notebook, NotePencil, Plus, Scribble, UploadSimple } from "@phosphor-icons/react";
import { DropdownMenu } from "radix-ui";
import { Button } from "@/components/ui/button";
import { shortcut } from "@/lib/platform";

export type NewKind = "folder" | "note" | "whiteboard" | "notebook" | "files" | "folders";

const ITEM = "flex cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5 text-[12.5px] outline-none data-highlighted:bg-accent";
const SEPARATOR = <DropdownMenu.Separator className="mx-1 my-1 h-px bg-border-subtle" />;

/** New: a folder, a note, a whiteboard or a notebook, or files and folders from
 *  the disk, all made in the folder on screen. Only drawn where the student
 *  may add. */
export function NewMenu({ busy, onPick }: { busy: boolean; onPick: (kind: NewKind) => void }) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button size="sm" disabled={busy} className="shrink-0">
          {busy ? <CircleNotch className="animate-spin" aria-hidden /> : <Plus aria-hidden />}
          {busy ? "Adding…" : "New"}
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          sideOffset={6}
          collisionPadding={12}
          className="z-50 min-w-52 rounded-xl border border-border-subtle bg-popover p-1 text-popover-foreground shadow-lg outline-none"
        >
          <Item icon={<FolderSimplePlus size={14} />} hint={shortcut("shift+mod+n")} onSelect={() => onPick("folder")}>New folder</Item>
          {SEPARATOR}
          <Item icon={<NotePencil size={14} className="text-reference-page" />} onSelect={() => onPick("note")}>Note</Item>
          <Item icon={<Scribble size={14} className="text-reference-page" />} onSelect={() => onPick("whiteboard")}>Whiteboard</Item>
          <Item icon={<Notebook size={14} className="text-reference-page" />} onSelect={() => onPick("notebook")}>Notebook…</Item>
          {SEPARATOR}
          <Item icon={<UploadSimple size={14} />} onSelect={() => onPick("files")}>Upload files…</Item>
          <Item icon={<FolderSimpleDashed size={14} />} onSelect={() => onPick("folders")}>Upload a folder…</Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function Item({ icon, hint, onSelect, children }: { icon: ReactNode; hint?: string; onSelect: () => void; children: ReactNode }) {
  return (
    <DropdownMenu.Item onSelect={onSelect} className={ITEM}>
      <span className="flex size-4 shrink-0 items-center justify-center text-muted-foreground">{icon}</span>
      <span className="flex-1">{children}</span>
      {hint && <span className="text-[11px] text-muted-foreground">{hint}</span>}
    </DropdownMenu.Item>
  );
}
