import { useEffect, useState } from "react";

import { LazyCodeEditor } from "@/components/code/LazyCodeEditor";
import type { EditorMemory } from "@/components/code/CodeEditor";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { readWorkspaceFile } from "@/lib/ed/workspace";
import type { WorkspaceSaver } from "@/lib/ed/workspaceSaver";

/**
 * One workspace file in `CodeEditor`. Its text is the saver's buffer: read
 * through `ed_workspace_read` the first time, kept while the tab is away,
 * and mirrored here so a change from elsewhere (another pane on the same
 * slide, a re-read) reaches the view. `remote` counts commits from another
 * client; each re-reads the file unless it has unsaved edits. `readOnly`
 * holds it while a conflict is being resolved.
 */
export function WorkspaceEditor({
  challengeId,
  saver,
  path,
  remote,
  memory,
  readOnly,
}: {
  challengeId: number;
  saver: WorkspaceSaver;
  path: string;
  remote: number;
  memory: EditorMemory;
  readOnly?: boolean;
}) {
  const [text, setText] = useState<string | null>(() => saver.text(path) ?? null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setErr(null);
    const off = saver.subscribe({ text: (p, t) => p === path && setText(t) });
    const held = saver.text(path);
    if (held !== undefined) {
      setText(held);
      return off;
    }
    setText(null);
    let live = true;
    readWorkspaceFile(challengeId, path)
      .then((t) => live && setText(saver.loaded(path, t)))
      .catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
      off();
    };
  }, [challengeId, saver, path]);

  useEffect(() => {
    if (!remote || saver.text(path) === undefined || saver.dirty(path)) return;
    let live = true;
    readWorkspaceFile(challengeId, path)
      .then((t) => live && saver.external(path, t))
      .catch((e) => console.warn(`Ed workspace ${challengeId}: ${path} not re-read:`, e));
    return () => {
      live = false;
    };
    // Only a new commit re-reads; a path change reads in the effect above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [remote]);

  if (err)
    return (
      <div className="px-4 py-3">
        <Alert variant="destructive">
          <AlertDescription className="text-xs">Failed to load {path}: {err}</AlertDescription>
        </Alert>
      </div>
    );
  if (text === null) return <LoadingFill />;
  return (
    <LazyCodeEditor
      text={text}
      filename={path}
      memory={memory}
      readOnly={readOnly}
      onChange={(next) => saver.edit(path, next)}
    />
  );
}
