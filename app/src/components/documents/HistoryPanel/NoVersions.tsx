import { shortcut } from "@/lib/platform";
import { cn } from "@/lib/utils";

/** What History says when the note has no versions yet. */
export function NoVersions({ className }: { className?: string }) {
  return (
    <div className={cn("text-center", className)}>
      <p className="text-[13px] text-foreground">No versions yet</p>
      <p className="mt-1 text-[12px] text-muted-foreground">
        Save a version ({shortcut("shift+mod+S")}) to keep the note as it is now. Snapshots are also
        taken as you write.
      </p>
    </div>
  );
}
