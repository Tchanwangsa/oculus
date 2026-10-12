import { useEffect, useRef } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

import type { TermSink, TermView } from "@/lib/ed/termOutput";
import { subscribeAppearance, subscribeTheme } from "@/lib/ui/themes";
import { cn } from "@/lib/utils";

import { xtermFont, xtermTheme } from "./xtermTheme";

export interface XtermViewProps {
  /** Where its output comes from; the view attaches on mount. */
  sink: TermSink;
  /** Typed text, as UTF-8. */
  onData(data: string): void;
  /** The fitted size, on mount and after every change. */
  onResize(cols: number, rows: number): void;
  label: string;
  className?: string;
}

/**
 * One xterm.js terminal filling its box: refitted on every resize of it
 * (`FitAddon`), themed from the app's tokens and code font. A box with no
 * size (a folded drawer) keeps the last fit.
 */
export function XtermView({ sink, onData, onResize, label, className }: XtermViewProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const latest = useRef({ onData, onResize });
  latest.current = { onData, onResize };

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const term = new Terminal({
      fontFamily: xtermFont(),
      fontSize: 12.5,
      lineHeight: 1.2,
      theme: xtermTheme(),
      scrollback: 5000,
      cursorBlink: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    term.textarea?.setAttribute("aria-label", label);

    let size: { cols: number; rows: number } | null = null;
    const refit = () => {
      if (host.clientWidth === 0 || host.clientHeight === 0) return;
      const dims = fit.proposeDimensions();
      if (!dims || !Number.isFinite(dims.cols) || !Number.isFinite(dims.rows)) return;
      fit.fit();
      if (size?.cols === term.cols && size.rows === term.rows) return;
      size = { cols: term.cols, rows: term.rows };
      latest.current.onResize(term.cols, term.rows);
    };
    refit();

    const view: TermView = {
      write: (data) => term.write(data),
      reset: () => term.reset(),
      size: () => size,
      focus: () => term.focus(),
    };
    sink.attach(view);
    const typed = term.onData((data) => latest.current.onData(data));
    const observer = new ResizeObserver(refit);
    observer.observe(host);
    // Tokens are rewritten after the prefs change; read them a frame later.
    let frame = 0;
    const retheme = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        term.options.theme = xtermTheme();
        term.options.fontFamily = xtermFont();
        refit();
      });
    };
    const offTheme = subscribeTheme(retheme);
    const offAppearance = subscribeAppearance(retheme);
    return () => {
      cancelAnimationFrame(frame);
      offTheme();
      offAppearance();
      observer.disconnect();
      typed.dispose();
      sink.detach(view);
      term.dispose();
    };
    // `label` only names the input once, at mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sink]);

  return <div ref={hostRef} data-selectable className={cn("h-full w-full overflow-hidden pl-3 pt-1.5", className)} />;
}
