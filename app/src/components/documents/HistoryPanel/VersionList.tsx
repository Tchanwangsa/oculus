import { SidebarSimple } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import type { DocumentVersion } from "@/lib/notes/documentVersions";

import { VersionListbox } from "./VersionListbox";

/** History's sidebar: a header that hides it (History stays open), then the
 *  rows, or skeletons while the first read is out. The page beside it says
 *  why there are none, or why the read failed. */
export function VersionList({
  versions,
  listError,
  selectedId,
  onSelect,
  renaming,
  onRelabel,
  onCancelRename,
  onHide,
}: {
  versions: DocumentVersion[] | null;
  listError: string | null;
  selectedId: number | null;
  onSelect: (id: number) => void;
  renaming: boolean;
  onRelabel: (v: DocumentVersion, label: string) => void;
  onCancelRename: () => void;
  onHide: () => void;
}) {
  return (
    <>
      <div className="flex h-9 shrink-0 items-center gap-2 border-b border-border-subtle pr-1.5 pl-3">
        <h2 className="min-w-0 flex-1 truncate text-[12px] font-medium text-foreground">History</h2>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Hide versions"
          title="Hide versions"
          className="text-muted-foreground"
          onClick={onHide}
        >
          <SidebarSimple size={14} mirrored />
        </Button>
      </div>

      {versions === null ? (
        !listError && <SkeletonRows count={4} rowClassName="h-10" className="p-3" />
      ) : versions.length === 0 ? (
        <p className="px-3 py-3 text-[12px] text-muted-foreground">No versions yet</p>
      ) : (
        <VersionListbox
          versions={versions}
          selectedId={selectedId}
          onSelect={onSelect}
          renaming={renaming}
          onRelabel={onRelabel}
          onCancelRename={onCancelRename}
        />
      )}
    </>
  );
}
