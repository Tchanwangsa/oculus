import type { KeyboardEvent, MouseEvent } from "react";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { fileIconFor } from "@/lib/files/fileTypes";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { attachmentPath, attachmentSrc, isImagePath } from "@/lib/harness/attachments";
import { attachmentKindLabel, useAttachmentLabel } from "@/stores/chat/attachmentNames";
import { useReferenceMissing } from "@/stores/chat/referenceStore";
import { cn } from "@/lib/utils";
import { ReferencePreview, referenceClass, referenceRestClass } from "./ReferencePreview";
import { StruckIcon, Unresolved, unresolvedClass } from "./Unresolved";

/** A label this long or shorter stays on one line, as `FileChip`'s does. */
const ONE_LINE = 40;

/** A long name cut in the middle, so its start and its extension both show. */
export function middleTruncate(name: string, max = ONE_LINE): string {
  if (name.length <= max) return name;
  const tail = Math.ceil((max - 1) / 2);
  return `${name.slice(0, max - 1 - tail)}…${name.slice(-tail)}`;
}

/**
 * A file attached to a message, drawn where the sentence names it: a glyph and
 * the name it was attached by (`attachmentNames`), on the wire its
 * `./attachments/…` path. In the composer (`onOpen` absent) it is atomic to
 * the caret and `data-attachment` is read back into the message; in a sent
 * bubble `data-path` is what a copy writes. A picture's hover shows its
 * thumbnail. A file gone from disk is unresolved (`referenceStore`).
 */
export function AttachmentChip({
  path,
  onOpen,
}: {
  /** The message's spelling of the path. */
  path: string;
  /** Omitted in the composer, where a click belongs to the caret. */
  onOpen?: () => void;
}) {
  const rel = attachmentPath(path) ?? path;
  const dataDir = useDataDir();
  const name = useAttachmentLabel(path);
  const unresolved = useReferenceMissing(rel);
  const picture = isImagePath(rel);
  const Icon = fileIconFor(rel);
  const { tone } = fileReferenceAppearance({ category: null, filename: rel });
  const label = middleTruncate(name);
  const where = onOpen ? { "data-path": path } : { "data-attachment": path, contentEditable: false };

  if (unresolved) {
    return (
      <Unresolved>
        <span {...where} className={cn("mx-px inline whitespace-nowrap box-decoration-clone", unresolvedClass)}>
          <StruckIcon icon={Icon} />
          {label}
        </span>
      </Unresolved>
    );
  }

  const icon = <Icon size="1.15em" aria-hidden className={cn("mr-[0.35em] inline align-[-0.18em]", tone)} />;
  const src = picture ? attachmentSrc(dataDir, rel) : "";
  const media = src ? (
    <img src={src} alt={name} className="block max-h-48 w-full rounded-lg border border-border bg-surface object-contain" />
  ) : undefined;
  return (
    <ReferencePreview icon={icon} title={name} kind={attachmentKindLabel(rel)} detail={path} media={media}>
      <span
        {...where}
        {...(onOpen
          ? {
              role: "button",
              tabIndex: 0,
              onClick: (e: MouseEvent) => {
                e.stopPropagation();
                if (window.getSelection()?.isCollapsed === false) return;
                onOpen();
              },
              onKeyDown: (e: KeyboardEvent) => {
                if (e.key !== "Enter" && e.key !== " ") return;
                e.preventDefault();
                e.stopPropagation();
                onOpen();
              },
            }
          : {})}
        className={cn(
          "mx-px inline whitespace-nowrap box-decoration-clone",
          onOpen ? cn(referenceClass, "cursor-pointer") : referenceRestClass,
        )}
      >
        {icon}
        {label}
      </span>
    </ReferencePreview>
  );
}
