import { useLayoutEffect, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Globe, MagnifyingGlass } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { faviconFor } from "@/hooks/shell/useBrowserTabs";
import { useBrowserStore } from "@/stores/shell/browserStore";
import type { Suggestion } from "./useBrowserAddressBar";

/**
 * The address field's suggestions, hung under the toolbar. A portal, so it
 * draws over the live page: inside `#root` it would fall in the page's hole
 * (`lib/shell/nativeHoles.ts`), and as a portal over the slot it also keeps
 * the page's clicks in the app while it is open.
 */
export function AddressSuggestions({
  anchorRef,
  suggestions,
  picked,
  setPicked,
  goTo,
}: {
  /** The toolbar the list hangs under, inset as `inset-x-2 top-full mt-1`. */
  anchorRef: RefObject<HTMLElement | null>;
  suggestions: Suggestion[];
  picked: number;
  setPicked: (i: number) => void;
  goTo: (url: string) => void;
}) {
  const favicons = useBrowserStore((s) => s.favicons);
  const [box, setBox] = useState<{ left: number; top: number; width: number } | null>(null);

  // Before paint, so the list never shows a frame at the origin.
  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const place = () => {
      const r = anchor.getBoundingClientRect();
      setBox({ left: r.left + 8, top: r.bottom + 4, width: r.width - 16 });
    };
    place();
    const observer = new ResizeObserver(place);
    observer.observe(anchor);
    window.addEventListener("resize", place);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", place);
    };
  }, [anchorRef]);

  if (!box) return null;

  return createPortal(
    <div
      style={box}
      className="fixed z-50 overflow-hidden rounded-lg border border-border bg-popover py-1 shadow-lg"
    >
      {suggestions.map((s, i) => {
        const icon = faviconFor(s.url, favicons);
        return (
          <button
            key={s.key}
            type="button"
            // `onMouseDown`, not `onClick`: the field's blur would close the
            // list before a click landed.
            onMouseDown={(e) => {
              e.preventDefault();
              goTo(s.url);
            }}
            onMouseEnter={() => setPicked(i)}
            className={cn(
              "flex w-full items-center gap-2.5 px-3 py-1.5 text-left",
              i === picked && "bg-sidebar-item-hover",
            )}
          >
            <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center text-muted-foreground">
              {s.kind === "search" ? (
                <MagnifyingGlass size={13} />
              ) : icon ? (
                <img
                  src={icon}
                  alt=""
                  className="h-3.5 w-3.5 rounded-[2px] object-contain"
                />
              ) : (
                <Globe size={13} />
              )}
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[12px] text-foreground">
                {s.label}
              </span>
              <span className="block truncate text-[11px] text-muted-foreground">
                {s.detail}
              </span>
            </span>
          </button>
        );
      })}
    </div>,
    document.body,
  );
}
