import { BackButton, VersionsToggle } from "./BarButtons";

/** The bar while no version is on the page (loading, empty or failed), so
 *  Back and the sidebar toggle stay where they are with a version. */
export function HistoryBar({
  title,
  onClose,
  listShown,
  onToggleList,
}: {
  /** The note's title. */
  title: string;
  onClose: () => void;
  listShown: boolean;
  onToggleList: () => void;
}) {
  return (
    <div className="@container/bar shrink-0 border-b border-border-subtle bg-card px-3 pt-2 pb-1.5">
      <div className="flex items-center gap-2">
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          <BackButton onClick={onClose} />
          <div className="min-w-0 flex-1">
            <p className="truncate font-display text-[15px] leading-tight font-semibold text-foreground">
              History
            </p>
            <p className="mt-0.5 truncate text-[11.5px] text-muted-foreground">{title.trim() || "Untitled"}</p>
          </div>
        </div>
        <VersionsToggle shown={listShown} onToggle={onToggleList} />
      </div>
    </div>
  );
}
