import { useEffect, useState, type KeyboardEvent, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { pageKey, rowStep, slideFrom, type SettingsPage } from "./pages";
import { useLayoutSize } from "./useLayoutSize";

/**
 * The settings popover's body: one page at a time, in place. The frame is
 * sized to the page showing and eases to the next page's size, so the
 * popover grows or shrinks around it instead of jumping. The page is pinned
 * to the frame's corner nearest the trigger (`data-side`, `data-align` on
 * the popover, read through the `group/settings` it carries), so the frame
 * reveals the page rather than sliding it. Until the first page is measured
 * it sits in flow and gives the frame its size.
 *
 * Each page takes focus as it shows (`focusPage`); Up and Down walk the
 * list's rows, Right opens one, Left goes back from a page. Escape is the
 * popover's, which calls `onBack` from a page (`pageKey`).
 */
export function PageFrame({
  page,
  from,
  slide,
  onBack,
  children,
}: {
  page: SettingsPage;
  /** The page the list was reached back from, whose row takes focus. */
  from: SettingsPage | null;
  /** The student has moved between pages since the popover opened, so the
   *  page slides in; the first page shows with the popover's own entrance. */
  slide: boolean;
  onBack: () => void;
  children: ReactNode;
}) {
  const [pageEl, setPageEl] = useState<HTMLDivElement | null>(null);
  const size = useLayoutSize(pageEl);

  useEffect(() => {
    if (pageEl) focusPage(pageEl, page, from);
  }, [pageEl, page, from]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (page === "root") {
      const rows = [...e.currentTarget.querySelectorAll<HTMLElement>("[data-settings-row]")];
      const at = rows.indexOf(target);
      if (at < 0) return;
      if (e.key === "ArrowRight") {
        e.preventDefault();
        target.click();
        return;
      }
      const next = rowStep(e.key, at, rows.length);
      if (next == null) return;
      e.preventDefault();
      rows[next].focus();
      return;
    }
    // Escape is handled by the popover (`onEscapeKeyDown`).
    if (e.key === "Escape") return;
    const editing = target.matches("input, textarea, [contenteditable=true]");
    if (pageKey(page, e.key, { editing, handled: e.defaultPrevented }) === "back") {
      e.preventDefault();
      onBack();
    }
  };

  const side = slideFrom(page);
  return (
    <div
      className="relative overflow-hidden transition-[width,height] duration-200 ease-out motion-reduce:transition-none"
      style={size ? { width: size.width, height: size.height } : undefined}
    >
      <div
        key={page}
        ref={setPageEl}
        onKeyDown={onKeyDown}
        className={cn(
          size &&
            "absolute group-data-[align=end]/settings:right-0 group-data-[align=start]/settings:left-0 group-data-[side=bottom]/settings:top-0 group-data-[side=top]/settings:bottom-0",
          slide &&
            "will-change-[opacity,transform] animate-in fade-in-0 duration-200 motion-reduce:animate-none",
          slide && (side === "right" ? "slide-in-from-right-3" : "slide-in-from-left-3"),
        )}
      >
        {children}
      </div>
    </div>
  );
}

/** Focus inside a page that just showed: on the list, the row for the page
 *  left, else the first row; a model page's search box, else its chosen
 *  model, else its first control; a mode page's chosen mode; the usage
 *  page's back control, its only one. */
function focusPage(el: HTMLElement, page: SettingsPage, from: SettingsPage | null) {
  const pick = (selector: string) => el.querySelector<HTMLElement>(selector);
  const target =
    page === "root"
      ? ((from && pick(`[data-settings-row="${from}"]`)) ?? pick("[data-settings-row]"))
      : page === "model"
        ? (pick("[data-model-search]") ?? pick('[aria-current="true"]') ?? pick("button:not([data-page-back])"))
        : page === "permission"
          ? pick('[aria-pressed="true"]')
          : pick("[data-page-back]");
  target?.focus({ preventScroll: true });
}
