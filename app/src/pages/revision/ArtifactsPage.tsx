import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { draftKey, useDraftStore } from "@/stores/chat/draftStore";
import { useNavigate } from "react-router-dom";
import { RevisionPage } from "@/components/revision/RevisionPage";
import { TopicPicker } from "@/components/revision/TopicPicker";
import { SourceLink } from "@/components/revision/SourceLink";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useRevisionData } from "@/layouts/RevisionLayout";
import {
  updateArtifact,
  deleteArtifact,
  type RevisionArtifact,
} from "@/lib/revision";
const OpenUIBlock = lazy(
  () => import("@/components/markdown/openui/OpenUIBlock"),
);
function Artifact({
  artifact,
  wide,
  onExpand,
}: {
  artifact: RevisionArtifact;
  wide: boolean;
  onExpand: () => void;
}) {
  const navigate = useNavigate();
  const [error, setError] = useState<string>();
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pending = useRef<Record<string, unknown> | null>(null);
  const flush = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    const state = pending.current;
    pending.current = null;
    if (state)
      void updateArtifact(artifact.id, { state }).catch((e) =>
        setError(String(e)),
      );
  }, [artifact.id]);
  useEffect(() => () => flush(), [flush]);
  const save = useCallback(
    (state: Record<string, unknown>) => {
      pending.current = state;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(flush, 400);
    },
    [flush],
  );
  return (
    <article
      className={`min-w-0 space-y-3 rounded-xl border border-border p-4 ${wide ? "col-span-full" : ""}`}
    >
      <Input
        aria-label="Artifact title"
        defaultValue={artifact.title}
        key={artifact.title}
        onBlur={(e) => {
          if (e.target.value.trim() && e.target.value !== artifact.title)
            void updateArtifact(artifact.id, {
              title: e.target.value.trim(),
            }).catch((e) => setError(String(e)));
        }}
      />
      <Suspense
        fallback={
          <p className="text-sm text-muted-foreground">Loading artifact…</p>
        }
      >
        <OpenUIBlock
          source={artifact.program}
          blockIndex={0}
          savedState={artifact.state}
          onPersistState={save}
          hideSave
          fallback={
            <p role="alert" className="text-sm text-destructive">
              This saved program cannot be rendered.
            </p>
          }
        />
      </Suspense>
      <div className="flex flex-wrap items-center gap-2">
        <TopicPicker
          subjectId={artifact.subject_id}
          value={artifact.topic_ids}
          onChange={(topic_ids) =>
            void updateArtifact(artifact.id, { topic_ids }).catch((e) =>
              setError(String(e)),
            )
          }
        />
        <SourceLink source={artifact.source_ref} />
      </div>
      <div className="flex flex-wrap gap-2">
        <Button size="xs" variant="outline" onClick={onExpand}>
          {wide ? "Show in grid" : "Expand"}
        </Button>
        <Button
          size="xs"
          variant="ghost"
          onClick={() => {
            const key = draftKey(null, "chat");
            const old = useDraftStore.getState().drafts[key] ?? "";
            useDraftStore
              .getState()
              .setDraft(
                key,
                [
                  old,
                  `Explain this interactive artifact: ${artifact.title}\n\n\`\`\`openui\n${artifact.program}\n\`\`\``,
                ]
                  .filter(Boolean)
                  .join("\n\n"),
              );
            navigate("/chat");
          }}
        >
          Ask about this
        </Button>
        <Button
          size="xs"
          variant="ghost"
          onClick={() =>
            void deleteArtifact(artifact.id).catch((e) => setError(String(e)))
          }
        >
          Delete
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
    </article>
  );
}
export default function ArtifactsPage() {
  const { artifacts } = useRevisionData();
  const [wide, setWide] = useState<number>();
  return (
    <RevisionPage title="Artifacts">
      {!artifacts.length && (
        <p className="py-10 text-center text-sm text-muted-foreground">
          Save an interactive widget from chat to keep exploring it here.
        </p>
      )}
      <div className="grid grid-cols-1 gap-4 @xl:grid-cols-2">
        {artifacts.map((artifact) => (
          <Artifact
            key={artifact.id}
            artifact={artifact}
            wide={wide === artifact.id}
            onExpand={() =>
              setWide(wide === artifact.id ? undefined : artifact.id)
            }
          />
        ))}
      </div>
    </RevisionPage>
  );
}
