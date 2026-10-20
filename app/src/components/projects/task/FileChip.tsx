import { ChipRemove } from "@/components/ui/ChipRemove";
import { displayCode } from "@/lib/format/format";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { fileTitle, openLibraryPath } from "@/lib/files/openFile";
import type { DbTaskFile } from "@/lib/planning/projects";

/**
 * A linked file as a chip: a button that opens it and, beside it, a button
 * that unlinks it. They are siblings, never nested (WebKit drops a nested
 * button's clicks). The × (`ChipRemove`) fades in on hover or keyboard focus
 * and floats over the chip's end, so revealing it moves nothing.
 *
 * Opening is `openLibraryPath`, the path every citation takes: the library's
 * own opener, so a PDF or note lands in the side panel and a binary in the
 * system viewer. ⌘-click and middle click open a tab.
 */
export function FileChip({ file, onRemove }: { file: DbTaskFile; onRemove: () => void }) {
  const { Icon, tone } = fileReferenceAppearance(file);
  const title = fileTitle(file);
  const code = file.subject_code ? displayCode(file.subject_code) : null;
  const open = (newTab: boolean) => openLibraryPath(file.relative_path, newTab);

  return (
    <span className="group relative inline-flex h-8 max-w-full min-w-0 items-center rounded-full border border-border-subtle transition-colors focus-within:bg-accent hover:bg-accent">
      <button
        type="button"
        title={file.relative_path}
        onClick={(e) => open(e.metaKey || e.ctrlKey)}
        onAuxClick={(e) => {
          if (e.button !== 1) return;
          e.preventDefault();
          open(true);
        }}
        className="flex h-full min-w-0 cursor-pointer items-center gap-2 rounded-full pr-3 pl-2.5 text-xs text-foreground outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
      >
        <Icon size={14} className={`shrink-0 ${tone}`} />
        <span className="truncate">{title}</span>
        {code && <span className="shrink-0 text-[11px] text-muted-foreground">{code}</span>}
      </button>
      <ChipRemove
        label={`Remove ${title}`}
        onRemove={onRemove}
        reveal="group"
        wrapperClassName="absolute top-[3px] right-[3px] flex size-6 items-center justify-center rounded-full bg-accent"
      />
    </span>
  );
}
