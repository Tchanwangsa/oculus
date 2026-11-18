import { useEffect, useState } from "react";
import { CheckIcon, CopyIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { NativeShareButton } from "./NativeShareButton";
import { copyText } from "@/lib/utils";

/** The selected link, selectable and clipped to one line, with Copy. It holds
 * the share's secret, so it is shown and copied, never logged. */
export function ShareLink({ link }: { link: string }) {
  const [copied, setCopied] = useState<"copied" | "failed" | null>(null);
  useEffect(() => { setCopied(null); }, [link]);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(null), 2000);
    return () => clearTimeout(timer);
  }, [copied]);
  const copy = async () => setCopied((await copyText(link)) ? "copied" : "failed");
  return (
    <div className="relative min-w-0 flex-1">
      {/* Keep the rounded clip composited as its buttons fade; see docs/ui.md. */}
      <div className="flex h-9 min-w-0 items-center overflow-hidden rounded-lg border border-input bg-card shadow-xs will-change-transform focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
        <input
          readOnly value={link} aria-label="Share link" spellCheck={false}
          onFocus={(event) => event.currentTarget.select()}
          className="h-full min-w-0 flex-1 bg-transparent px-3 text-xs text-foreground outline-none select-text"
        />
        <Button size="icon-sm" variant="ghost" onClick={() => void copy()} aria-live="polite"
          aria-label={copied === "copied" ? "Link copied" : copied === "failed" ? "Copy failed" : "Copy link"}
          title={copied === "copied" ? "Copied" : copied === "failed" ? "Copy failed" : "Copy link"}
          className="rounded-none border-l border-border-subtle text-muted-foreground will-change-[opacity]">
          {copied === "copied" ? <CheckIcon /> : <CopyIcon />}
        </Button>
        <NativeShareButton link={link} />
      </div>
    </div>
  );
}
