import { useRef, useState } from "react";
import { DownloadSimple } from "@phosphor-icons/react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { isWaiting } from "@/lib/browser/downloads/types";
import { cn } from "@/lib/utils";
import { useBrowserDownloadsStore } from "@/stores/shell/browserDownloadsStore";
import { DownloadRow } from "./DownloadRow";

/**
 * The toolbar's downloads button, shown once there has been a download this
 * session, with a dot while one waits for a choice. Its popover lists every
 * download, newest first; the list is the popover's one scroller.
 */
export function DownloadsButton({ className }: { className: string }) {
  const seen = useBrowserDownloadsStore((s) => s.seen);
  const list = useBrowserDownloadsStore((s) => s.downloads);
  const ask = useBrowserDownloadsStore((s) => s.ask);
  const [open, setOpen] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);
  // Set while the popover closes to open the dialog, so focus is left to it.
  const asking = useRef(false);
  useScrollFade(scroller, "y", open ? list.length : -1);

  if (!seen) return null;
  const waiting = list.some(isWaiting);
  const newest = [...list].reverse();

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          aria-label={waiting ? "Downloads, one waiting for a choice" : "Downloads"}
          title="Downloads"
          className={cn(className, "relative")}
        >
          <DownloadSimple size={15} />
          {waiting ? (
            <span aria-hidden className="absolute top-1 right-1 size-1.5 rounded-full bg-brand" />
          ) : null}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="flex w-80 flex-col p-0"
        onCloseAutoFocus={(e) => {
          if (asking.current) e.preventDefault();
          asking.current = false;
        }}
      >
        <div className="shrink-0 px-3 pt-2.5 pb-1.5 text-[12px] font-medium text-foreground">Downloads</div>
        {newest.length ? (
          <div
            ref={scroller}
            className="max-h-[min(24rem,calc(var(--radix-popover-content-available-height)-3rem))] overflow-y-auto pb-1"
          >
            <div className="divide-y divide-border-subtle">
              {newest.map((d) => (
                <DownloadRow
                  key={d.id}
                  download={d}
                  onAsk={(id) => {
                    asking.current = true;
                    setOpen(false);
                    ask(id);
                  }}
                />
              ))}
            </div>
          </div>
        ) : (
          <p className="px-3 pb-3 text-[12px] text-muted-foreground">Nothing downloaded this session.</p>
        )}
      </PopoverContent>
    </Popover>
  );
}
