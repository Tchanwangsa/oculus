import { useEffect, useState } from "react";
import { Archive, ArrowCounterClockwise, DotsThree, PencilSimple, Trash } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
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
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { DbProject } from "@/lib/projects";

/**
 * The three things you can do to a project that are not editing its board:
 * rename it, put it away, and destroy it.
 *
 * Built on Popover rather than a dropdown-menu primitive because there is no
 * such component in `components/ui` and this is the second menu of this shape
 * — `StatusPill` is the first, and its row styling is copied here on purpose
 * so a menu is a menu wherever it opens.
 *
 * It owns **no** store calls: every item hands back through a callback and the
 * page decides what a rename or a delete means where it is standing. That is
 * what lets one component serve the index, a subject's tab and the project's
 * own header without any of them being the shape it was written for. It does
 * own the dialogs, though — a caller that had to mount the confirmation itself
 * would be a caller that could forget to.
 */
export function ProjectMenu({
  project,
  onRename,
  onArchive,
  onUnarchive,
  onDelete,
  className,
  align = "end",
}: {
  project: DbProject;
  /** The new name, already trimmed and known to differ from the current one —
   *  a no-op rename never reaches here. */
  onRename: (name: string) => void;
  onArchive: () => void;
  onUnarchive: () => void;
  /** Takes the project's tasks with it. The confirmation has already been
   *  given by the time this fires. */
  onDelete: () => void;
  className?: string;
  /** Passed to the popover: a row's menu hangs off its right edge, a header's
   *  may not. */
  align?: React.ComponentProps<typeof PopoverContent>["align"];
}) {
  const [open, setOpen] = useState(false);
  /**
   * Which dialog the menu is on its way to opening, if any.
   *
   * A single value rather than a flag each, because the menu can only ever be
   * leaving for one of them — and because the popover's closing autofocus has
   * to know that it is leaving for a dialog at all (see `onCloseAutoFocus`).
   */
  const [pending, setPending] = useState<"rename" | "delete" | null>(null);
  const archived = project.status === "archived";

  const leaveFor = (next: "rename" | "delete") => {
    setPending(next);
    setOpen(false);
  };

  return (
    <>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Actions for ${project.name}`}
            title="Actions"
            className={cn("text-muted-foreground hover:text-foreground", className)}
          >
            <DotsThree size={16} weight="bold" />
          </Button>
        </PopoverTrigger>
        <PopoverContent
          align={align}
          className="w-44 p-1"
          /* Closing a popover hands focus back to its trigger, and a dialog
             opening in the same tick traps focus into itself: the two fight
             and the field can end up unfocused behind the overlay. When the
             close is *because* an item opened a dialog, the trigger does not
             get the focus back — the dialog does. */
          onCloseAutoFocus={(e) => {
            if (pending) e.preventDefault();
          }}
        >
          <MenuItem icon={<PencilSimple size={12} />} onClick={() => leaveFor("rename")}>
            Rename
          </MenuItem>
          {archived ? (
            <MenuItem
              icon={<ArrowCounterClockwise size={12} />}
              onClick={() => {
                setOpen(false);
                onUnarchive();
              }}
            >
              Unarchive
            </MenuItem>
          ) : (
            <MenuItem
              icon={<Archive size={12} />}
              onClick={() => {
                setOpen(false);
                onArchive();
              }}
            >
              Archive
            </MenuItem>
          )}
          <MenuItem
            icon={<Trash size={12} />}
            destructive
            onClick={() => leaveFor("delete")}
          >
            Delete
          </MenuItem>
        </PopoverContent>
      </Popover>

      <RenameDialog
        project={project}
        open={pending === "rename"}
        onOpenChange={(next) => !next && setPending(null)}
        onRename={onRename}
      />

      <Dialog open={pending === "delete"} onOpenChange={(next) => !next && setPending(null)}>
        <DialogContent className="sm:max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Delete “{project.name}”?</DialogTitle>
            {/* Said plainly, because this is the one table in the app nothing
                upstream has a copy of: a sync can put a Canvas file back and
                nothing can put this back. The tasks are named because the row
                you clicked is the project, and the cascade is not visible from
                here. */}
            <DialogDescription>
              Its tasks and subtasks go with it. This is your own planning — nothing syncs it
              back, and there is no archive to find it in afterwards. Archive it instead if you
              only want it off the list.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setPending(null)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                setPending(null);
                onDelete();
              }}
            >
              Delete project
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

/** A row of the menu. `StatusPill`'s picker rows, with a leading glyph and a
 *  destructive reading for the one item that cannot be undone. */
function MenuItem({
  icon,
  destructive = false,
  onClick,
  children,
}: {
  icon: React.ReactNode;
  destructive?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors",
        destructive
          ? "text-destructive hover:bg-destructive/10"
          : "text-foreground hover:bg-accent",
      )}
    >
      <span className="shrink-0 opacity-70">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{children}</span>
    </button>
  );
}

/**
 * Rename, as a dialog with one field.
 *
 * A name is the one thing about a project you change from a list row, where
 * there is nothing to edit in place — the row is a link, and turning it into a
 * field would mean the whole row stops being one. A dialog also gives Escape
 * a meaning without any handler of its own.
 *
 * The draft is seeded on every *opening*, keyed off `open` rather than the
 * project, so re-opening after an abandoned edit starts from the stored name
 * again instead of resuming the text you walked away from.
 */
function RenameDialog({
  project,
  open,
  onOpenChange,
  onRename,
}: {
  project: DbProject;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRename: (name: string) => void;
}) {
  const [draft, setDraft] = useState(project.name);

  useEffect(() => {
    if (open) setDraft(project.name);
  }, [open, project.name]);

  const commit = () => {
    const name = draft.trim();
    // An empty field and an untouched one are both "never mind", not a write:
    // a project with no name is unfindable in every list it appears in, and a
    // rename to the name it already has would still bump `updated_at` and
    // reorder anything sorted by it.
    if (name && name !== project.name) onRename(name);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Rename project</DialogTitle>
        </DialogHeader>
        {/* A form, so Enter commits the way it does in every other single-field
            box — no keydown handler, and the confirm button is its submit. */}
        <form
          onSubmit={(e) => {
            e.preventDefault();
            commit();
          }}
          className="flex flex-col gap-4"
        >
          <Input
            autoFocus
            value={draft}
            aria-label="Project name"
            placeholder="Project name"
            onChange={(e) => setDraft(e.target.value)}
          />
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit">Rename</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
