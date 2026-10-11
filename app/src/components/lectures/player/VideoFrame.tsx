import { cn } from "@/lib/utils";
import type { SourceNum } from "@/lib/db";
import { SourceSwitcher, type SourceStates } from "@/components/lectures/SourceControls";

/** One picture: the box a long-lived `<video>` is moved into
 *  (`lib/lectures/playback/`), plus its source switcher. */
export function VideoFrame({
  hostRef,
  source,
  sources,
  showSwitcher,
  chromeVisible,
  pinned,
  onSelectSource,
  onDownloadSource,
  onSwitcherOpenChange,
  onPointerDown,
  className,
  style,
  children,
}: {
  hostRef: React.RefObject<HTMLDivElement | null>;
  source: SourceNum;
  sources: SourceStates;
  /** False for a single-stream capture: nothing to switch to. */
  showSwitcher: boolean;
  /** The control bar is showing, so frame chrome may show too. */
  chromeVisible: boolean;
  /** This frame's switcher is open, so it stays put whatever the pointer does. */
  pinned: boolean;
  onSelectSource: (source: SourceNum) => void;
  onDownloadSource: (source: SourceNum) => void;
  onSwitcherOpenChange: (open: boolean) => void;
  /** Set on the PIP, where the whole box is the drag handle. */
  onPointerDown?: (e: React.PointerEvent) => void;
  className?: string;
  style?: React.CSSProperties;
  children?: React.ReactNode;
}) {
  return (
    <div
      className={cn("group/frame relative min-h-0 min-w-0 overflow-hidden", className)}
      style={style}
      onPointerDown={onPointerDown}
    >
      {/* Empty on purpose: the shared element is moved in here. */}
      <div ref={hostRef} className="h-full w-full" />

      {showSwitcher && (
        <div
          className={cn(
            "absolute left-2 top-2 z-20 transition-opacity will-change-[opacity] duration-150",
            pinned
              ? "opacity-100"
              : chromeVisible
                ? "opacity-0 group-hover/frame:opacity-100"
                : "pointer-events-none opacity-0",
          )}
          // Inside the PIP the whole box is a drag handle; the pill is not.
          onPointerDown={(e) => e.stopPropagation()}
        >
          <SourceSwitcher
            active={source}
            states={sources}
            onSelect={onSelectSource}
            onDownload={onDownloadSource}
            onOpenChange={onSwitcherOpenChange}
          />
        </div>
      )}

      {children}
    </div>
  );
}
