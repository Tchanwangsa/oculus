import { useCallback } from "react";
import { cn } from "@/lib/utils";
import type { TranscriptSearch } from "@/hooks/lectures/useTranscriptSearch";
import { fmtClockSecs, type Cue } from "@/lib/lectures/media";
import { FollowList } from "@/components/media/FollowList";
import { Highlight } from "@/components/media/follow/Highlight";
import { SearchField } from "@/components/media/follow/SearchField";

export interface TranscriptListProps {
  cues: Cue[];
  activeCueIdx: number;
  /** `useTranscriptSearch` over `cues`, held by the dock so a query outlives
   *  a visit to another tab. */
  search: TranscriptSearch;
  /** The dock is open — the list stays mounted either way. */
  open: boolean;
  /** This tab is in front. */
  active: boolean;
  onSeek: (seconds: number) => void;
  /** The list is tracking playback rather than being read by hand. */
  following: boolean;
  /** A hand-scroll pushed the playing cue out of frame — stop following. */
  onScrollAway: () => void;
  /** Resume following and snap back to the playing cue. */
  onBackToLive: () => void;
}

/** The Transcript tab's cue list: search, then cue space mapped to
 *  `FollowList` row space. */
export function TranscriptList({
  cues,
  activeCueIdx,
  search,
  open,
  active,
  onSeek,
  following,
  onScrollAway,
  onBackToLive,
}: TranscriptListProps) {
  const { query, setQuery, needle, searching, rows, followIdx } = search;
  // The virtualizer keys its measurement memo by this function identity.
  const getCueKey = useCallback((row: number) => rows[row], [rows]);

  return (
    <>
      <SearchField
        value={query}
        onChange={setQuery}
        placeholder="Search"
        count={searching ? rows.length : undefined}
      />
      <FollowList
        count={rows.length}
        followIdx={followIdx}
        // Keyed by cue index so measured heights survive filtering.
        getItemKey={getCueKey}
        open={open}
        active={active}
        following={following}
        onScrollAway={onScrollAway}
        onBackToLive={onBackToLive}
        resetKey={needle}
        overlay={searching && rows.length === 0 ? "No matches" : undefined}
        renderRow={(row, item, measure) => {
          const cueIdx = rows[row];
          const cue = cues[cueIdx];
          const playing = cueIdx === activeCueIdx;
          return (
            <button
              data-index={item.index}
              ref={measure}
              onClick={() => onSeek(cue.start)}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${item.start}px)`,
              }}
              className={cn(
                "text-left text-[11px] px-2 py-1 rounded flex gap-2 items-start",
                playing
                  ? "bg-brand/12 text-brand"
                  : "text-muted-foreground hover:text-foreground hover:bg-surface",
              )}
            >
              <span className="tabular-nums text-[10px] shrink-0 pt-px w-10 opacity-60">
                {fmtClockSecs(Math.floor(cue.start))}
              </span>
              <span className="flex-1">
                <Highlight text={cue.text} needle={needle} />
              </span>
            </button>
          );
        }}
      />
    </>
  );
}
