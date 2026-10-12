import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "@phosphor-icons/react";

/** Caption controls sit outside the tab strip's measured width. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const window = getCurrentWindow();
    let disposed = false;
    const refresh = () => window.isMaximized().then((value) => {
      if (!disposed) setMaximized(value);
    }).catch((cause) => console.error("Window state failed", cause));
    void refresh();
    const listening = window.onResized(() => { void refresh(); });
    return () => { disposed = true; void listening.then((unlisten) => unlisten()).catch((cause) => console.error("Window resize listener failed", cause)); };
  }, []);
  const run = (action: () => Promise<void>) => {
    setError(null);
    void action().catch((cause) => setError(String(cause)));
  };
  const button = "flex h-11 w-10 shrink-0 items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors";
  return (
    <div className="relative -mr-2 flex shrink-0 self-stretch">
      <button type="button" aria-label="Minimize window" className={button} onClick={() => run(() => getCurrentWindow().minimize())}><Minus size={14} /></button>
      <button type="button" aria-label={maximized ? "Restore window" : "Maximize window"} className={button} onClick={() => run(() => getCurrentWindow().toggleMaximize())}>{maximized ? <Copy size={13} /> : <Square size={13} />}</button>
      <button type="button" aria-label="Close window" className={`${button} hover:bg-destructive hover:text-white`} onClick={() => run(() => getCurrentWindow().close())}><X size={16} /></button>
      {error && <p role="alert" data-selectable className="absolute right-0 top-full z-50 w-64 rounded-lg border border-border bg-card p-3 text-xs text-destructive">{error}</p>}
    </div>
  );
}
