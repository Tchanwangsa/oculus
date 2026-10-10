import { useCallback } from "react";
import { NoteField } from "@/components/documents/NoteField";
import {
  attachmentPath,
  attachmentSrc,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  writeAttachment,
} from "@/lib/harness/attachments";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { libraryImageSrc } from "@/lib/files/libraryLinks";
import { taskBodyEdit, type DbProject, type DbProjectTask } from "@/lib/planning/projects";

/**
 * The description: markdown in a `NoteField`, so the note editor's live
 * preview, maths, tables, shortcuts and toolbar; `@` searches the project's
 * subject or the whole library. Pictures go to `agents/attachments/` on
 * arrival. Blur and ⌘↵ save, unchanged writes nothing, empty writes `null`.
 * Keyed by task (below), so leaving a task saves to that task.
 */
export function TaskBody({
  task,
  project,
  onSave,
}: {
  task: DbProjectTask;
  project: DbProject | null;
  onSave: (body: string | null) => void;
}) {
  const dataDir = useDataDir();
  /** An attachment in either spelling, else a path from the data dir. */
  const imageSrc = useCallback(
    (src: string) => {
      const picture = attachmentPath(src);
      return picture ? attachmentSrc(dataDir, picture) : libraryImageSrc(src, "", dataDir);
    },
    [dataDir],
  );

  const writePicture = useCallback(async (source: File | string) => {
    const pending = typeof source === "string" ? pendingFromPath(source) : pendingFromFile(source);
    try {
      return await writeAttachment(pending);
    } finally {
      // No preview strip here, so the preview URL is dead once written.
      releaseAttachment(pending);
    }
  }, []);

  return (
    <NoteField
      className="mt-6"
      text={task.body ?? ""}
      subjectId={project?.subject_id ?? null}
      imageSrc={imageSrc}
      writePicture={writePicture}
      pickerTitle="Add pictures to this task"
      notAPicture="Only images can go in a task body — use @ for a course file."
      placeholder="Write what this actually involves…"
      label="Description"
      onCommit={(text) => {
        const body = taskBodyEdit(text, task.body);
        if (body !== undefined) onSave(body);
      }}
    />
  );
}
