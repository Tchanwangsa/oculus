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
 * Rename / archive / delete. Makes no store calls — callers act through the
 * callbacks — but owns the rename and delete-confirm dialogs so no caller can
 * skip them. Row styling matches `StatusPill`'s picker.
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
  /** Trimmed, non-empty and changed. */
  onRename: (name: string) => void;
  onArchive: () => void;
  onUnarchive: () => void;
  /** Fires after confirmation. */
  onDelete: () => void;
  className?: string;
  align?: React.ComponentProps<typeof PopoverContent>["align"];
}) {
  const [open, setOpen] = useState(false);
  /** The dialog the menu is closing to open (read by `onCloseAutoFocus`). */
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
          /* Don't return focus to the trigger when closing into a dialog, or
             it fights the dialog's focus trap. */
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

/** The draft reseeds on every opening, so an abandoned edit doesn't resume. */
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
    // Empty or unchanged is a cancel: a same-name write would still bump
    // `updated_at`.
    if (name && name !== project.name) onRename(name);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Rename project</DialogTitle>
        </DialogHeader>
        {/* A form, so Enter submits. */}
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
