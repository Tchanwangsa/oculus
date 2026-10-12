import { ArrowClockwise, CaretDown, CaretUp } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { PillTabs } from "@/components/ui/table/PillTabs";
import type { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { cn } from "@/lib/utils";

import { LazyXtermView } from "./LazyXtermView";
import { TerminalOverlay } from "./TerminalOverlay";
import type { useRunSession } from "./useRunSession";
import type { useTerminalSession } from "./useTerminalSession";

export type DrawerTab = "terminal" | "run";

const LABELS: Record<DrawerTab, string> = { terminal: "Terminal", run: "Run" };

/**
 * Ed's bottom drawer under the editor: a Terminal tab (Ed's bash) and a Run
 * tab (Run's output), each an xterm view kept mounted behind the other so
 * neither loses its scrollback. Drag its top edge to resize, or fold it to
 * its tab bar; the views stay laid out at the unfolded height meanwhile, so
 * a fold never resizes Ed's PTY.
 */
export function TerminalDrawer({
  panel,
  tabs,
  tab,
  onTab,
  terminal,
  run,
  offline,
}: {
  panel: ReturnType<typeof useResizablePanel>;
  tabs: DrawerTab[];
  tab: DrawerTab;
  onTab: (tab: DrawerTab) => void;
  terminal: ReturnType<typeof useTerminalSession>;
  run: ReturnType<typeof useRunSession>;
  /** Why Ed can't be reached now; null when online. */
  offline: string | null;
}) {
  const pick = (next: DrawerTab) => {
    onTab(next);
    panel.setCollapsed(false);
  };
  const restartable = tab === "terminal" && (terminal.phase === "exited" || terminal.phase === "failed");
  return (
    <section aria-label="Terminal and output" className="flex shrink-0 flex-col border-t border-border-subtle">
      <ResizeHandle
        orientation="horizontal"
        onMouseDown={panel.onMouseDown}
        dragging={panel.dragging}
        label="Resize terminal"
        className="-mt-0.5 -mb-0.5"
      />
      <div className="flex h-9 shrink-0 items-center gap-2 px-3">
        <PillTabs tabs={tabs.map((t) => ({ value: t, label: LABELS[t] }))} value={tab} onChange={pick} />
        <div className="flex-1" />
        {restartable && (
          <Button size="xs" variant="ghost" onClick={terminal.restart}>
            <ArrowClockwise aria-hidden />
            {terminal.phase === "failed" ? "Try again" : "New terminal"}
          </Button>
        )}
        <Button
          size="icon-xs"
          variant="ghost"
          onClick={panel.toggle}
          aria-label={panel.collapsed ? "Show the drawer" : "Hide the drawer"}
          title={panel.collapsed ? "Show" : "Hide"}
        >
          {panel.collapsed ? <CaretUp aria-hidden /> : <CaretDown aria-hidden />}
        </Button>
      </div>
      <div
        inert={panel.collapsed}
        style={{ height: panel.width }}
        className={cn("overflow-hidden", !panel.dragging && "transition-[height] duration-200 ease-out")}
      >
        <div style={{ height: panel.restWidth }} className="relative">
          {tabs.includes("terminal") && (
            <div className={cn("absolute inset-0", tab !== "terminal" && "invisible")}>
              <LazyXtermView sink={terminal.sink} onData={terminal.input} onResize={terminal.resize} label="Terminal" />
              <TerminalOverlay terminal={terminal} offline={offline} />
            </div>
          )}
          {tabs.includes("run") && (
            <div className={cn("absolute inset-0", tab !== "run" && "invisible")}>
              <LazyXtermView sink={run.sink} onData={run.input} onResize={run.resize} label="Program input" />
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

