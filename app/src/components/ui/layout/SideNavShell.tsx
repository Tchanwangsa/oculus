import { useLayoutEffect, useRef, type ReactNode } from "react";
import { X } from "@phosphor-icons/react";
import type { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import { cn } from "@/lib/utils";

/** Shared page frame: dock on wide panes, cover the content on narrow ones. */
export function SideNavShell({ panel, nav, children, className }: {
  panel: ReturnType<typeof useSideNavPanel>;
  nav: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  const drawer = useRef<HTMLDivElement>(null);
  const showing = panel.overlay && !panel.collapsed;

  useLayoutEffect(() => {
    if (!showing) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    drawer.current?.querySelector<HTMLButtonElement>("[data-drawer-close]")?.focus();
    return () => {
      if (previous?.isConnected && previous.getClientRects().length) previous.focus();
    };
  }, [showing]);

  return (
    <div ref={panel.ref} data-side-nav-shell className={cn("relative isolate flex h-full min-w-0 overflow-hidden", className)}>
      {showing && (
        <button
          type="button"
          tabIndex={-1}
          aria-label="Close sidebar"
          onClick={panel.dismiss}
          className="absolute inset-0 z-20 bg-foreground/15"
        />
      )}
      <div
        ref={drawer}
        data-side-nav-overlay={panel.overlay || undefined}
        inert={panel.collapsed}
        onKeyDown={(event) => {
          if (showing && event.key === "Escape" && !event.defaultPrevented) {
            event.preventDefault();
            event.stopPropagation();
            panel.dismiss();
          }
        }}
        onClick={(event) => {
          if (showing && event.target instanceof Element && event.target.closest("a[href], [data-tab-href], [data-side-nav-close]")) panel.dismiss();
        }}
        className={cn(
          "relative flex h-full shrink-0",
          panel.overlay && "absolute inset-y-0 left-0 z-30 max-w-[calc(100%-2rem)] overflow-hidden [&>nav]:max-w-full [&>[role=separator]]:hidden",
          showing && "bg-background shadow-xl",
        )}
      >
        {nav}
        {showing && (
          <button
            type="button"
            data-drawer-close
            aria-label="Close sidebar"
            onClick={panel.dismiss}
            className="absolute top-1 right-1 flex size-6 items-center justify-center rounded-md bg-background text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <X size={14} />
          </button>
        )}
      </div>
      <div inert={showing} className="flex h-full min-w-0 flex-1 flex-col">
        {children}
      </div>
    </div>
  );
}
