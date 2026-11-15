import type { ComponentProps } from "react";
import { useReferenceMissing } from "@/stores/chat/referenceStore";
import { FileChip } from "./FileChip";

/** A student's `@` mention (in the composer or a sent bubble): `FileChip`,
 *  unresolved once the file has left the library (`referenceStore`). It reads
 *  its own status, so marking it redraws this chip alone, never the
 *  composer's editable text. */
export function CheckedFileChip(props: Omit<ComponentProps<typeof FileChip>, "unresolved">) {
  const missing = useReferenceMissing(props.path);
  return <FileChip {...props} unresolved={missing} />;
}
