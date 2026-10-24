import { useState } from "react";
import { NoteField } from "@/components/documents/NoteField";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { libraryImageSrc } from "@/lib/files/libraryLinks";
import {
  attachmentPath,
  attachmentSrc,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  writeAttachment,
} from "@/lib/harness/attachments";
export function MarkdownField({
  text,
  subjectId = null,
  label = "Body",
  onCommit,
  onChange,
  compact,
  inline,
}: {
  text: string;
  subjectId?: number | null;
  label?: string;
  onCommit: (text: string) => void;
  onChange?: (text: string) => void;
  compact?: boolean;
  inline?: boolean;
}) {
  const dataDir = useDataDir();
  const [error, setError] = useState<string>();
  const writePicture = async (source: File | string) => {
    const pending =
      typeof source === "string"
        ? pendingFromPath(source)
        : pendingFromFile(source);
    try {
      return await writeAttachment(pending);
    } finally {
      releaseAttachment(pending);
    }
  };
  return (
    <div className="min-w-0">
      <NoteField
        text={text}
        subjectId={subjectId}
        compact={compact}
        inline={inline}
        imageSrc={(src) => {
          const path = attachmentPath(src);
          return path
            ? attachmentSrc(dataDir, path)
            : libraryImageSrc(src, "", dataDir);
        }}
        writePicture={writePicture}
        pickerTitle="Add picture"
        notAPicture="Choose an image."
        placeholder="Write a definition, formula or example…"
        label={label}
        onChange={onChange}
        onCommit={(value) => {
          try {
            onCommit(value);
          } catch (e) {
            setError(String(e));
          }
        }}
      />
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
