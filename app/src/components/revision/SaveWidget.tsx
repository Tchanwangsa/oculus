import { useState, useContext } from "react";
import { useStateField } from "@openuidev/react-lang";
import { WidgetIndex } from "@/components/markdown/openui/state";
import { invoke } from "@tauri-apps/api/core";
import { useWidgetRow } from "@/components/markdown/openui/state";
import { widgetSubject } from "@/lib/revision/subject";
import {
  createArtifact,
  notifyRevisionUpdated,
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
export function SaveArtifact({
  source,
  state,
}: {
  source: string;
  state: () => Record<string, unknown>;
}) {
  const row = useWidgetRow();
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("Interactive artifact");
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  return (
    <div
      data-copy-skip
      className="flex flex-wrap items-center justify-end gap-2 text-xs"
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
                  createArtifact({
                    title: title.trim(),
                    program: source,
                    state: state(),
                    subject_id: subject,
                    source_ref: row ? `${row.thread}:${row.row}` : null,
                  }),
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
