import { CircleNotch } from "@phosphor-icons/react";
import { SubjectLabel } from "@/components/markdown/openui/proposals/SubjectLabel";
import { stateWords, type Tone } from "@/lib/browser/downloads/describe";
import type { Download } from "@/lib/browser/downloads/types";
import { cn } from "@/lib/utils";

const TONE: Record<Tone, string> = {
  muted: "text-muted-foreground",
  brand: "text-brand",
  success: "text-success",
  error: "text-destructive",
};

/** A download's state in words, with its subject drawn as the app draws one
 *  (icon and short code) or its folder named. */
export function DownloadState({ download }: { download: Download }) {
  const words = stateWords(download);
  return (
    <div className="min-w-0 text-[11px] leading-snug">
      <div className={cn("flex min-w-0 items-center gap-1", TONE[words.tone])}>
        {download.state === "delivering" ? (
          <CircleNotch size={11} className="shrink-0 animate-spin" aria-hidden />
        ) : null}
        <span className="shrink-0">{words.text}</span>
        {words.place?.kind === "subject" ? (
          <SubjectLabel code={words.place.code} className="h-4.5" />
        ) : words.place?.kind === "folder" ? (
          <span className="min-w-0 truncate text-foreground" title={words.place.name}>
            {words.place.name}
          </span>
        ) : null}
      </div>
      {words.detail ? (
        <p className={cn("break-words", words.tone === "error" ? "text-destructive" : "text-muted-foreground")}>
          {words.detail}
        </p>
      ) : null}
    </div>
  );
}
