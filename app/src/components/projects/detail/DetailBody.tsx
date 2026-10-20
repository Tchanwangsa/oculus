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
import { taskBodyEdit } from "@/lib/planning/projects";

/** Markdown and attachment handling shared by project About and task descriptions. */
export function DetailBody({ text, subjectId, kind, onSave, className = "-mx-2 mt-5" }: {
  text: string | null;
  subjectId: number | null;
  kind: "task" | "project";
  onSave: (body: string | null) => void;
  className?: string;
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
      // The field's own 8px inset puts its text under the title's edge.
      className={className}
      text={text ?? ""}
      subjectId={subjectId}
      imageSrc={imageSrc}
      writePicture={writePicture}
      pickerTitle={`Add pictures to this ${kind}`}
      notAPicture="Only images can go here — use @ for a course file."
      placeholder={kind === "project" ? "What is this project?" : "Add a description…"}
      label={kind === "project" ? "About" : "Description"}
      onCommit={(next) => {
        const body = taskBodyEdit(next, text);
        if (body !== undefined) onSave(body);
      }}
    />
  );
}
