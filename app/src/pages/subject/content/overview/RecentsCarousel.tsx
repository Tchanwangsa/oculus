import { useLayoutEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { CaretLeft, CaretRight, FileText, VideoCamera } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { relativeTime, type RecentEntry } from "@/lib/activity/recents";
import { openFileSmart } from "@/lib/files/openFile";
import type { DbFile } from "@/lib/db";

/** Card row scrolled by flanking arrows, each shown only while there is more
 *  that way. */
export function RecentsCarousel({
  recents, files, subjectId,
}: {
  recents: RecentEntry[];
  files: DbFile[];
  subjectId: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);

  const update = () => {
    const el = ref.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 1);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1);
  };

  useLayoutEffect(() => {
    update();
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [recents]);

  const scrollBy = (dir: 1 | -1) => {
    const el = ref.current;
    if (!el) return;
    el.scrollBy({ left: dir * el.clientWidth * 0.8, behavior: "smooth" });
  };

  return (
    <div className="relative">
      <div
        ref={ref}
        onScroll={update}
        className="flex gap-2.5 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {recents.map((entry) => (
          <RecentCard
            key={`${entry.kind}:${entry.ref}`}
            entry={entry}
            files={files}
            subjectId={subjectId}
          />
        ))}
      </div>

      {canLeft && <CarouselArrow side="left" onClick={() => scrollBy(-1)} />}
      {canRight && <CarouselArrow side="right" onClick={() => scrollBy(1)} />}
    </div>
  );
}

function CarouselArrow({
  side, onClick,
}: {
  side: "left" | "right";
  onClick: () => void;
}) {
  const Icon = side === "left" ? CaretLeft : CaretRight;
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={side === "left" ? "Scroll back" : "Scroll forward"}
      className={cn(
        "absolute top-1/2 -translate-y-1/2 z-10 flex h-7 w-7 items-center justify-center rounded-full",
        "border border-border bg-background shadow-sm text-muted-foreground",
        "hover:text-foreground hover:bg-surface transition-colors",
        side === "left" ? "-left-3.5" : "-right-3.5",
      )}
    >
      <Icon size={12} />
    </button>
  );
}

function RecentCard({
  entry, files, subjectId,
}: {
  entry: RecentEntry;
  files: DbFile[];
  subjectId: number;
}) {
  const navigate = useNavigate();
  const Icon = entry.kind === "lecture" ? VideoCamera : FileText;

  const open = () => {
    if (entry.kind === "lecture") {
      navigate(
        `/subjects/${subjectId}/lecture?id=${encodeURIComponent(entry.ref)}&t=${encodeURIComponent(entry.title)}`,
      );
      return;
    }
    const file = files.find((f) => f.relative_path === entry.ref);
    if (file) openFileSmart(file);
  };

  return (
    <button
      onClick={open}
      className="shrink-0 w-44 rounded-lg border border-border px-3 py-2.5 text-left hover:bg-surface transition-colors"
    >
      <Icon size={15} className="text-muted-foreground" />
      <p className="mt-2 text-[12px] font-medium text-foreground line-clamp-2 break-words leading-snug h-[2.75em]">
        {entry.title}
      </p>
      <p className="mt-1.5 text-[11px] text-muted-foreground">
        {relativeTime(entry.visitedAt)}
      </p>
    </button>
  );
}
