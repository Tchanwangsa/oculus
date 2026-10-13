import { X } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import type { DocumentVersion } from "@/lib/notes/documentVersions";

import { NoVersions } from "./NoVersions";
import { PanelError } from "./PanelError";
import { VersionListbox } from "./VersionListbox";

/** The list side of History, wherever the tier puts it (docked, floating, or
 *  the whole pane): a header that closes History, then the rows, skeletons
 *  while the first read is out, or why there are none — unless the page
 *  beside it already says so (`emptyCopy` false). */
export function VersionList({
  versions,
  listError,
  selectedId,
  onSelect,
  onOpen,
  renaming,
  onRelabel,
  onCancelRename,
  onClose,
  emptyCopy,
}: {
  versions: DocumentVersion[] | null;
  listError: string | null;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onOpen?: (id: number) => void;
  renaming: boolean;
  onRelabel: (v: DocumentVersion, label: string) => void;
  onCancelRename: () => void;
  onClose: () => void;
  emptyCopy: boolean;
}) {
  return (
    <>
      <div className="flex h-9 shrink-0 items-center gap-2 border-b border-border-subtle pr-1.5 pl-3">
        <h2 className="min-w-0 flex-1 truncate text-[12px] font-medium text-foreground">History</h2>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Close history"
          title="Close history"
          className="text-muted-foreground"
          onClick={onClose}
        >
          <X size={13} />
        </Button>
      </div>

      {listError && <PanelError message={listError} />}

      {versions === null ? (
        !listError && <SkeletonRows count={4} rowClassName="h-10" className="p-3" />
      ) : versions.length === 0 ? (
        emptyCopy && <NoVersions className="px-4 py-8" />
      ) : (
        <VersionListbox
          versions={versions}
          selectedId={selectedId}
          onSelect={onSelect}
          onOpen={onOpen}
          renaming={renaming}
          onRelabel={onRelabel}
          onCancelRename={onCancelRename}
        />
      )}
    </>
  );
}
