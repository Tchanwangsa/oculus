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
    <div className="flex min-w-0 flex-wrap items-center gap-2">
      <input
        readOnly value={link} aria-label="Share link" spellCheck={false}
        onFocus={(event) => event.currentTarget.select()}
        className="h-8 min-w-40 flex-1 truncate rounded-md border border-border bg-transparent px-2.5 text-xs text-foreground outline-none select-text focus-visible:ring-[3px] focus-visible:ring-ring/50"
      />
      <Button size="sm" variant="outline" onClick={() => void copy()} aria-live="polite">
        {copied === "copied" ? <CheckIcon /> : <CopyIcon />}
        {copied === "copied" ? "Copied" : copied === "failed" ? "Copy failed" : "Copy link"}
      </Button>
      <NativeShareButton link={link} />
    </div>
  );
}
