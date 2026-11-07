import { lazy, Suspense } from "react";
import { HighlightedCode } from "@/components/markdown/HighlightedCode";
import { LiveWidget } from "@/components/markdown/widget/LiveWidget";
import type { RevisionArtifact } from "@/lib/revision";
import { artifactWidgetKey } from "@/stores/chat/widgetStore";

const OpenUIBlock = lazy(
  () => import("@/components/markdown/openui/OpenUIBlock"),
);

/** The artifact as the fence it came from, for Ask about this and copying. */
export const artifactFence = (artifact: RevisionArtifact) =>
  `\`\`\`${artifact.format}\n${artifact.program}\n\`\`\``;

/**
 * A saved artifact drawn by its format's runtime: OpenUI through the lazy
 * `OpenUIBlock`, a widget through `LiveWidget` under its own store key, so
 * its Allow is asked afresh and never shared with the reply it came from.
 * Both take their state from the row and hand changes to `onPersistState`.
 * No hooks here, so tests can call it.
 */
export function ArtifactBody({
  artifact,
  onPersistState,
}: {
  artifact: RevisionArtifact;
  onPersistState: (state: Record<string, unknown>) => void;
}) {
  if (artifact.format === "widget") {
    return (
      <LiveWidget
        // A changed program is a new widget: its frame and state start over.
        key={artifact.program}
        source={artifact.program}
        title={artifact.title}
        md={artifactFence(artifact)}
        storeKey={artifactWidgetKey(artifact.id)}
        code={
          <pre className="my-3">
            <code className="block overflow-x-auto rounded-lg bg-surface-raised p-3 font-mono text-[13px] text-foreground">
              <HighlightedCode info="tsx" code={artifact.program} />
            </code>
          </pre>
        }
        savedState={artifact.state}
        onPersistState={onPersistState}
        hideSave
      />
    );
  }
  return (
    <Suspense
      fallback={
        <p className="text-sm text-muted-foreground">Loading artifact…</p>
      }
    >
      <OpenUIBlock
        source={artifact.program}
        blockIndex={0}
        savedState={artifact.state}
        onPersistState={onPersistState}
        hideSave
        fallback={
          <p role="alert" className="text-sm text-destructive">
            This saved program cannot be rendered.
          </p>
        }
      />
    </Suspense>
  );
}
