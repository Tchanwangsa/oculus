import { useEffect, useState } from "react";
import { CaretRight, Star } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import {
  cancelWhisperDownload,
  deleteWhisperModel,
  downloadWhisperModel,
  onWhisperModelProgress,
  pickWhisperModel,
  type TranscribeSettings,
  type WhisperCatalogue,
  type WhisperModel,
} from "@/lib/lectures/transcribe";

type WhisperSettings = TranscribeSettings["whisper"];

/** Decimal, as Hugging Face and whisper.cpp quote these sizes. */
function size(bytes: number): string {
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.round(bytes / 1e6)} MB`;
}

/** Binary, as a Mac's memory is sold: 36 GB is 36 × 2³⁰. */
function machineLine(catalogue: WhisperCatalogue): string {
  const ram = catalogue.totalRamBytes
    ? `${Math.round(catalogue.totalRamBytes / 2 ** 30)} GB`
    : "memory unknown";
  return `This Mac: ${ram}, ${catalogue.gpu ? "Apple GPU" : "CPU only"}`;
}

export type Download = { received: number; total: number };

function percent(download: Download): number {
  return download.total > 0 ? Math.min(100, Math.floor((download.received / download.total) * 100)) : 0;
}

/**
 * Model downloads in flight and their failures, held by the page rather than
 * the dialog so the engine row's status follows a download after the dialog
 * closes. Progress arrives as `whisper-model-progress` events; a download
 * started on an earlier visit shows at 0 until its next event.
 */
export function useWhisperDownloads(catalogue: WhisperCatalogue | null, onModelsChanged: () => void) {
  const [downloads, setDownloads] = useState<Record<string, Download>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!catalogue) return;
    setDownloads((current) => {
      const next = { ...current };
      for (const m of catalogue.models) {
        if (m.downloading && !next[m.id]) next[m.id] = { received: 0, total: m.bytes };
      }
      return next;
    });
  }, [catalogue]);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void onWhisperModelProgress((p) => {
      if (p.phase === "downloading") {
        setDownloads((d) => ({ ...d, [p.id]: { received: p.received, total: p.total } }));
        return;
      }
      setDownloads(({ [p.id]: _, ...rest }) => rest);
      if (p.phase === "error" && p.error) setErrors((e) => ({ ...e, [p.id]: p.error! }));
      if (p.phase === "complete") onModelsChanged();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [onModelsChanged]);

  const setError = (id: string, error: string | null) =>
    setErrors(({ [id]: _, ...rest }) => (error ? { ...rest, [id]: error } : rest));

  const download = (model: WhisperModel) => {
    setError(model.id, null);
    setDownloads((d) => ({ ...d, [model.id]: { received: 0, total: model.bytes } }));
    downloadWhisperModel(model.id).catch((cause) => {
      const text = String(cause);
      setDownloads(({ [model.id]: _, ...rest }) => rest);
      if (text !== "cancelled") setError(model.id, text);
    });
  };

  const cancel = (model: WhisperModel) => {
    cancelWhisperDownload(model.id).catch((cause) => {
      console.error("Whisper download cancel failed", cause);
    });
  };

  return { downloads, errors, download, cancel, setError };
}

export type WhisperDownloads = ReturnType<typeof useWhisperDownloads>;

/** The engine row's one line: what a run would use, or what is missing. */
export function whisperStatus(
  catalogue: WhisperCatalogue | null,
  settings: WhisperSettings | null,
  downloads: Record<string, Download>,
): { text: string; ready: boolean } {
  if (!catalogue || !settings) return { text: "", ready: false };
  const inFlight = catalogue.models.find((m) => downloads[m.id]);
  const active = pickWhisperModel(catalogue, settings.model);
  const ready = catalogue.helper && active !== null;
  if (inFlight) {
    return { text: `Downloading ${inFlight.label} · ${percent(downloads[inFlight.id])}%`, ready };
  }
  if (!catalogue.helper) return { text: "The Whisper helper is missing", ready };
  if (settings.model && !active) {
    const chosen = catalogue.models.find((m) => m.id === settings.model);
    return { text: `${chosen?.label ?? settings.model} is not downloaded`, ready };
  }
  return { text: active?.label ?? "No model downloaded", ready };
}

/**
 * The local Whisper engine's model files: Automatic or a chosen model, the
 * recommended one first and the rest folded under "Other models". The
 * catalogue is local to read; only Download touches the network.
 */
export function LocalWhisperDialog({
  open, onOpenChange, catalogue, settings, downloads, onChange, onModelsChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** null until the listing has answered. */
  catalogue: WhisperCatalogue | null;
  /** null until the `transcribe` row has been read. */
  settings: WhisperSettings | null;
  downloads: WhisperDownloads;
  onChange: (patch: Partial<WhisperSettings>) => void;
  /** A model was deleted: list again. */
  onModelsChanged: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Local Whisper</DialogTitle>
          <DialogDescription>
            whisper.cpp on this Mac
            {catalogue?.gpu === false ? ", on the CPU" : ", on the GPU through Metal"}: free and
            offline after a one-time model download.
          </DialogDescription>
        </DialogHeader>
        {catalogue && settings ? (
          <Models
            catalogue={catalogue}
            settings={settings}
            downloads={downloads}
            onChange={onChange}
            onModelsChanged={onModelsChanged}
          />
        ) : (
          <p className="text-xs text-muted-foreground">Reading the models…</p>
        )}
      </DialogContent>
    </Dialog>
  );
}

function Models({ catalogue, settings, downloads, onChange, onModelsChanged }: {
  catalogue: WhisperCatalogue;
  settings: WhisperSettings;
  downloads: WhisperDownloads;
  onChange: (patch: Partial<WhisperSettings>) => void;
  onModelsChanged: () => void;
}) {
  const [confirm, setConfirm] = useState<WhisperModel | null>(null);
  const [deleting, setDeleting] = useState(false);

  const recommended = catalogue.models.find((m) => m.fit === "recommended") ?? null;
  const others = catalogue.models.filter((m) => m !== recommended);
  const picked = pickWhisperModel(catalogue, null);
  // Folded unless something there is already in play.
  const [showOthers, setShowOthers] = useState(() =>
    others.some((m) => m.downloaded || m.downloading || m.id === settings.model),
  );

  const remove = async (model: WhisperModel) => {
    setDeleting(true);
    try {
      await deleteWhisperModel(model.id);
      // A chosen model that is gone would fail every run; let `pick` choose.
      if (settings.model === model.id) onChange({ model: null });
      downloads.setError(model.id, null);
      onModelsChanged();
    } catch (cause) {
      console.error("Whisper model delete failed", cause);
      downloads.setError(model.id, String(cause));
    } finally {
      setDeleting(false);
      setConfirm(null);
    }
  };

  const row = (model: WhisperModel) => (
    <ModelRow
      key={model.id}
      model={model}
      selected={settings.model === model.id}
      inUse={settings.model === null && picked?.id === model.id}
      download={downloads.downloads[model.id] ?? null}
      error={downloads.errors[model.id] ?? null}
      onSelect={() => onChange({ model: model.id })}
      onDownload={() => downloads.download(model)}
      onCancel={() => downloads.cancel(model)}
      onDelete={() => setConfirm(model)}
    />
  );

  return (
    <div className="min-w-0">
      <p className="text-[11px] text-muted-foreground">
        {machineLine(catalogue)}
        {settings.enabled ? "" : " · Switched off, so runs skip it"}
      </p>
      {catalogue.helper ? null : (
        <p className="mt-1 text-[11px] leading-relaxed text-warning">
          The Whisper helper is missing — run <span className="text-foreground">bun run whisper</span> in app/.
        </p>
      )}

      <div role="radiogroup" aria-label="Whisper model" className="mt-2">
        <div className="flex items-center gap-2.5 py-1.5">
          <Radio label="Automatic" checked={settings.model === null} onSelect={() => onChange({ model: null })} />
          <div className="min-w-0 flex-1">
            <p className="text-xs text-foreground">Automatic</p>
            <p className="text-[11px] text-muted-foreground">
              {picked ? `Uses ${picked.label}` : "Picks a model once one is downloaded"}
            </p>
          </div>
        </div>

        {recommended ? row(recommended) : null}

        {others.length ? (
          <>
            <button
              type="button"
              aria-expanded={showOthers}
              onClick={() => setShowOthers((v) => !v)}
              className="mt-1 flex items-center gap-1 py-1 text-[11px] text-muted-foreground hover:text-foreground"
            >
              <CaretRight
                size={10}
                weight="bold"
                className={cn("transition-transform", showOthers && "rotate-90")}
              />
              Other models
            </button>
            {showOthers ? others.map(row) : null}
          </>
        ) : null}
      </div>

      <ConfirmDialog
        open={confirm !== null}
        onCancel={() => setConfirm(null)}
        onConfirm={() => confirm && void remove(confirm)}
        title={`Delete ${confirm?.label ?? "this model"}?`}
        description={
          confirm
            ? `Frees ${size(confirm.bytes)}. It can be downloaded again from here.`
            : ""
        }
        confirmLabel="Delete"
        busyLabel="Deleting…"
        busy={deleting}
      />
    </div>
  );
}

/** A radio dot, or the space it takes, so labels line up. */
function Radio({ label, checked, onSelect, hidden = false }: {
  label: string;
  checked: boolean;
  onSelect: () => void;
  hidden?: boolean;
}) {
  return (
    <span className="flex size-4 shrink-0 items-center justify-center">
      {hidden ? null : (
        <button
          type="button"
          role="radio"
          aria-checked={checked}
          aria-label={label}
          onClick={onSelect}
          className={cn(
            "flex size-3.5 items-center justify-center rounded-full border transition-colors",
            "outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
            checked ? "border-brand" : "border-input hover:border-muted-foreground",
          )}
        >
          {checked ? <span className="size-1.5 rounded-full bg-brand" /> : null}
        </button>
      )}
    </span>
  );
}

function ModelRow({
  model, selected, inUse, download, error, onSelect, onDownload, onCancel, onDelete,
}: {
  model: WhisperModel;
  /** Chosen by name. */
  selected: boolean;
  /** What Automatic picks right now. */
  inUse: boolean;
  download: Download | null;
  error: string | null;
  onSelect: () => void;
  onDownload: () => void;
  onCancel: () => void;
  onDelete: () => void;
}) {
  const done = download ? percent(download) : 0;

  return (
    <div className="py-1.5">
      <div className="flex items-center gap-2.5">
        <Radio label={model.label} checked={selected} onSelect={onSelect} hidden={!model.downloaded} />

        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-1.5 text-xs text-foreground">
            <span className="truncate">{model.label}</span>
            {model.fit === "recommended" ? (
              <span className="inline-flex shrink-0 items-center gap-0.5 text-[11px] text-brand">
                <Star size={10} weight="fill" aria-hidden />
                Recommended
              </span>
            ) : null}
          </p>
          <p className="text-[11px] text-muted-foreground tabular-nums">
            {size(model.bytes)} download · {size(model.ramBytes)} memory
            {model.fit === "too_large" ? " · Needs more memory than this Mac has to spare" : ""}
            {inUse ? " · In use" : ""}
          </p>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {download ? (
            <>
              <span className="w-9 text-right text-[11px] text-brand tabular-nums">{done}%</span>
              <Button variant="outline" size="xs" onClick={onCancel}>
                Cancel
              </Button>
            </>
          ) : model.downloaded ? (
            <Button variant="ghost" size="xs" className="text-muted-foreground" onClick={onDelete}>
              Delete
            </Button>
          ) : (
            <Button variant="outline" size="xs" onClick={onDownload}>
              Download
            </Button>
          )}
        </div>
      </div>

      {download ? (
        <Progress value={done} className="mt-1.5 ml-6.5 h-1 w-auto" indicatorClassName="bg-brand" />
      ) : null}
      {error ? (
        <p className="mt-1 ml-6.5 text-[11px] leading-relaxed text-destructive" data-selectable>
          {error}
        </p>
      ) : null}
    </div>
  );
}
