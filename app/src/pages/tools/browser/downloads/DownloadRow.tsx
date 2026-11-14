import { useState, type ReactNode } from "react";
import { X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { downloads } from "@/lib/browser/downloads/commands";
import { sizeWords } from "@/lib/browser/downloads/describe";
import { isSettled, isWaiting, type Download } from "@/lib/browser/downloads/types";
import { fileIconFor } from "@/lib/files/fileTypes";
import { DownloadState } from "./DownloadState";
import { openInOculus } from "./openInOculus";
import { problem, REVEAL } from "./words";

interface Action {
  label: string;
  run: () => Promise<unknown>;
}

/** What can be done with a download that is not waiting for a choice. One
 *  that failed or was refused offers nothing: its address is not kept (a
 *  signed link, a form post or a `blob:` cannot be fetched a second time),
 *  so it is started again from its page. */
function actionsFor(d: Download): Action[] {
  const saved = d.state === "saved" ? d.saved_to : null;
  if (saved?.kind === "oculus") {
    return [
      { label: "Open in Oculus", run: () => openInOculus(saved) },
      { label: REVEAL, run: () => downloads.reveal(d.id) },
    ];
  }
  if (saved?.kind === "path") {
    return [
      { label: REVEAL, run: () => downloads.reveal(d.id) },
      { label: "Open", run: () => downloads.open(d.id) },
    ];
  }
  return [];
}

/** The icon and the text column every row shares. */
function Details({ download, children }: { download: Download; children?: ReactNode }) {
  const Icon = fileIconFor(download.name);
  const size = sizeWords(download);
  return (
    <>
      <Icon size={14} className="mt-0.5 shrink-0 opacity-60" aria-hidden />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate text-[12px] text-foreground" title={download.name}>
          {download.name}
        </span>
        <span className="flex min-w-0 gap-1 text-[11px] text-muted-foreground">
          {download.host ? <span className="min-w-0 truncate">{download.host}</span> : null}
          {download.host && size ? <span aria-hidden>·</span> : null}
          {size ? <span className="shrink-0 tabular-nums">{size}</span> : null}
        </span>
        <DownloadState download={download} />
        {children}
      </span>
    </>
  );
}

/**
 * One download in the toolbar's list: its name, where it came from, its size
 * and its state in words, with what can be done with it now. A row waiting
 * for a choice is itself the button that asks again.
 */
export function DownloadRow({ download, onAsk }: { download: Download; onAsk: (id: number) => void }) {
  const [error, setError] = useState<string | null>(null);

  if (isWaiting(download)) {
    return (
      <button
        type="button"
        onClick={() => onAsk(download.id)}
        title="Choose where to save it"
        className="flex w-full items-start gap-2.5 px-3 py-2 text-left outline-none transition-colors hover:bg-accent focus-visible:bg-accent"
      >
        <Details download={download} />
      </button>
    );
  }

  const act = (action: () => Promise<unknown>) => {
    setError(null);
    action().catch((e) => setError(problem(e)));
  };
  const actions = actionsFor(download);

  return (
    <div className="flex items-start gap-2.5 px-3 py-2">
      <Details download={download}>
        {actions.length ? (
          <span className="mt-1 flex flex-wrap gap-1">
            {actions.map((a) => (
              <Button key={a.label} variant="secondary" size="xs" onClick={() => act(a.run)}>
                {a.label}
              </Button>
            ))}
          </span>
        ) : null}
        {error ? <span role="alert" className="text-[11px] text-destructive">{error}</span> : null}
      </Details>
      {isSettled(download) ? (
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={`Clear ${download.name} from the list`}
          title="Clear from the list"
          onClick={() => act(() => downloads.clear(download.id))}
          className="-mr-1 shrink-0 text-muted-foreground hover:text-foreground"
        >
          <X aria-hidden />
        </Button>
      ) : null}
    </div>
  );
}
