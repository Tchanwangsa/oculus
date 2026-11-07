import { useState, useContext } from "react";
import { useStateField } from "@openuidev/react-lang";
import { WidgetIndex } from "@/components/markdown/openui/state";
import { invoke } from "@tauri-apps/api/core";
import { useWidgetRow } from "@/components/markdown/openui/state";
import { widgetSubject } from "@/lib/revision/subject";
import { artifactInput } from "@/lib/revision/artifacts";
import { cn } from "@/lib/utils";
import {
  createArtifact,
  notifyRevisionUpdated,
  type ArtifactFormat,
  type ItemInput,
} from "@/lib/revision";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
export function SaveQuestions({ items }: { items: ItemInput[] }) {
  const row = useWidgetRow();
  const index = useContext(WidgetIndex);
  const savedField = useStateField<number[] | null>(
    `revisionSaved:${index}`,
    null,
  );
  const saved = savedField.value != null;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  return (
    <span className="flex flex-wrap items-center gap-2">
      <Button
        variant="ghost"
        size="xs"
        disabled={busy || saved}
        onClick={() => {
          setBusy(true);
          void widgetSubject(row?.thread ?? null)
            .then((subject) =>
              invoke<number[]>("revision_save_items", {
                items: items.map((item) => ({
                  ...item,
                  subject_id: subject,
                  status: "draft",
                  created_by: "agent",
                })),
              }),
            )
            .then((ids) => {
              savedField.setValue(ids);
              notifyRevisionUpdated();
            })
            .catch((e) => setError(String(e)))
            .finally(() => setBusy(false));
        }}
      >
        {saved ? "Saved as drafts" : busy ? "Saving…" : "Save to Revision"}
      </Button>
      {error && (
        <span role="alert" className="text-xs text-destructive">
          {error}
        </span>
      )}
    </span>
  );
}
/** Save to Revision for a fence in a reply: names it, then writes its
 *  program and `state()` as it is at that moment. */
export function SaveArtifact({
  source,
  state,
  format = "openui",
  defaultTitle = "Interactive artifact",
  className,
}: {
  source: string;
  state: () => Record<string, unknown>;
  format?: ArtifactFormat;
  defaultTitle?: string;
  className?: string;
}) {
  const row = useWidgetRow();
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState(defaultTitle);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  return (
    <div
      data-copy-skip
      className={cn(
        "flex flex-wrap items-center justify-end gap-2 text-xs",
        className,
      )}
    >
      {!open ? (
        <Button
          variant="ghost"
          size="xs"
          disabled={saved}
          onClick={() => setOpen(true)}
        >
          {saved ? "Saved to Revision" : "Save to Revision"}
        </Button>
      ) : (
        <>
          <Input
            aria-label="Artifact title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className="h-7 w-48"
          />
          <Button
            size="xs"
            disabled={busy || !title.trim()}
            onClick={() => {
              setBusy(true);
              void widgetSubject(row?.thread ?? null)
                .then((subject) =>
                  createArtifact(
                    artifactInput({
                      title,
                      program: source,
                      format,
                      state: state(),
                      subject,
                      row,
                    }),
                  ),
                )
                .then(() => {
                  setSaved(true);
                  setOpen(false);
                })
                .catch((e) => setError(String(e)))
                .finally(() => setBusy(false));
            }}
          >
            Save
          </Button>
          <Button variant="ghost" size="xs" onClick={() => setOpen(false)}>
            Cancel
          </Button>
        </>
      )}
      {error && (
        <span role="alert" className="text-destructive">
          {error}
        </span>
      )}
    </div>
  );
}
