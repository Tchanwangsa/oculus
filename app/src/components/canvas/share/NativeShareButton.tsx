import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ShareFatIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { isMac } from "@/lib/platform";

/** AppKit owns destination choice, transfer progress and cancellation. The
 * command stays pending until dismissal, so a second picker cannot open. */
export function NativeShareButton({ link }: { link: string }) {
  const supported = isMac && isTauri();
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = useRef(0);
  const busy = useRef(false);
  useEffect(() => {
    setError(null);
    setOpening(busy.current);
    return () => {
      current.current++;
      if (supported) void invoke("native_share_close").catch(() => {});
    };
  }, [link, supported]);

  if (!supported) return null;
  const share = async (button: HTMLButtonElement) => {
    if (busy.current) return;
    busy.current = true;
    const request = ++current.current;
    const rect = button.getBoundingClientRect();
    setOpening(true);
    setError(null);
    try {
      await invoke("native_share_link", {
        link,
        anchor: { x: rect.x, y: rect.y, width: rect.width, height: rect.height, viewportWidth: window.innerWidth, viewportHeight: window.innerHeight },
      });
    } catch (failure) {
      if (current.current === request) setError(typeof failure === "string" ? failure : "Could not open the macOS share sheet. Try again or copy the link.");
    } finally {
      busy.current = false;
      setOpening(false);
    }
  };
  return (
    <>
      <Button size="icon-sm" variant="ghost" disabled={opening}
        onMouseDown={(event) => { if (event.button === 0) void share(event.currentTarget); }}
        onClick={(event) => { if (event.detail === 0) void share(event.currentTarget); }}
        aria-label="Share link using macOS" title={opening ? "Share sheet open…" : "Share link using macOS"}
        className="rounded-none border-l border-border-subtle text-muted-foreground">
        <ShareFatIcon />
      </Button>
      {error && <p role="alert" className="absolute top-full right-0 z-10 mt-1 max-w-full rounded-md border border-border bg-card p-2 text-xs text-destructive shadow-sm">{error}</p>}
    </>
  );
}
