import { CircleNotch, Trash, VideoCamera } from "@phosphor-icons/react";
import { ProviderMark } from "@/components/icons/ProviderMark";
import {
  SIDE_NAV_FOLDS,
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_ACTIVE,
  SIDE_NAV_ROW_IDLE,
} from "@/components/ui/layout/SideNav";
import { chatHref, type HarnessThread } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { ConfirmDelete } from "./parts";

/** A thread as a nav row: its provider's mark is the icon a fold leaves, and a
 *  running thread marks that icon's corner while folded. */
export function ThreadRow({
  thread: t,
  active,
  running,
  confirming,
  onOpen,
  onArm,
  onDelete,
}: {
  thread: HarnessThread;
  active: boolean;
  running: boolean;
  confirming: boolean;
  onOpen: (id: number) => void;
  /** Arms this row's delete, or disarms with null. */
  onArm: (id: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const title = t.title?.trim() || "Untitled";
  return (
    <div
      className={cn(
        SIDE_NAV_ROW,
        "group/thread relative gap-0 overflow-hidden p-0",
        active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE,
      )}
    >
      {/* Padding lives inside the button so the whole lit row is the hit target. */}
      <button
        type="button"
        // ⌘-click opens the thread in a new tab (`app/src/lib/shell/newTabClicks.ts`).
        data-tab-href={chatHref(t.id, t.title)}
        onClick={() => onOpen(t.id)}
        className="flex min-w-0 flex-1 items-center gap-2.5 py-1.5 pl-2 text-left"
      >
        <ProviderMark provider={t.provider} className="size-4 shrink-0" />
        <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{title}</span>
        {t.lecture_id && (
          <VideoCamera
            size={11}
            className={cn("shrink-0 opacity-60", SIDE_NAV_FOLDS)}
            aria-label="Lecture thread"
          />
        )}
      </button>
      <div className={cn("flex shrink-0 items-center pr-1.5 pl-1", SIDE_NAV_FOLDS)}>
        {running ? (
          <CircleNotch size={12} className="animate-spin text-muted-foreground" aria-label="Running" />
        ) : confirming ? (
          <ConfirmDelete
            label="Yes"
            onConfirm={() => {
              onArm(null);
              onDelete(t.id);
            }}
            onCancel={() => onArm(null)}
          />
        ) : (
          <button
            type="button"
            aria-label="Delete thread"
            onClick={() => onArm(t.id)}
            className="rounded p-0.5 opacity-0 transition-opacity will-change-[opacity] hover:text-foreground focus-visible:opacity-100 group-hover/thread:opacity-100 group-data-[collapsed=true]/nav:pointer-events-none"
          >
            <Trash size={12} />
          </button>
        )}
      </div>
      {running && (
        <span
          aria-hidden
          className="absolute top-1 left-5 size-1.5 rounded-full bg-brand opacity-0 transition-opacity will-change-[opacity] duration-150 group-data-[collapsed=true]/nav:opacity-100"
        />
      )}
    </div>
  );
}
