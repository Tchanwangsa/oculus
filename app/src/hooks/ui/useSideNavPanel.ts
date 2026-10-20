import { useCallback, useLayoutEffect, useState } from "react";
import { useLocation } from "react-router-dom";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { useNarrowBox } from "@/hooks/ui/useNarrowBox";
import { activePane } from "@/stores/shell/tabStore";

/** The drawer's temporary state never overwrites the desktop width or fold. */
export function useSideNavPanel(options: Parameters<typeof useResizablePanel>[0]) {
  const desktop = useResizablePanel(options);
  const [ref, overlay] = useNarrowBox(768);
  const [open, setOpen] = useState(false);
  const location = useLocation();
  const active = useTabActive();
  const paneId = useTabId();

  useLayoutEffect(() => setOpen(false), [overlay, location.key, active]);

  const dismiss = useCallback(() => setOpen(false), []);
  const toggle = useCallback(() => {
    if (overlay) setOpen((value) => !value);
    else desktop.toggle();
  }, [overlay, desktop.toggle]);
  const setCollapsed = useCallback((collapsed: boolean) => {
    if (overlay) setOpen(!collapsed);
    else desktop.setCollapsed(collapsed);
  }, [overlay, desktop.setCollapsed]);

  useWindowEvent("keydown", (event) => {
    const key = event as KeyboardEvent;
    if (!active || key.defaultPrevented || (paneId !== 0 && activePane()?.id !== paneId)) return;
    if (!key.altKey || !(key.metaKey || key.ctrlKey) || key.code !== "KeyB") return;
    key.preventDefault();
    toggle();
  });

  return {
    ...desktop,
    ref,
    overlay,
    dismiss,
    collapsed: overlay ? !open : desktop.collapsed,
    width: overlay ? (open ? desktop.restWidth : 0) : desktop.width,
    toggle,
    setCollapsed,
  };
}
