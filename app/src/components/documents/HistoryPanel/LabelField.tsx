import { useRef, useState } from "react";
import { BookmarkSimple } from "@phosphor-icons/react";

import { Input } from "@/components/ui/input";
import type { DocumentVersion } from "@/lib/notes/documentVersions";

/** The selected checkpoint's label, edited in its row: Enter or leaving the
 *  field saves (blank clears it), Esc keeps the old one. */
export function LabelField({
  version,
  onDone,
  onCancel,
}: {
  version: DocumentVersion;
  onDone: (label: string) => void;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState(version.label ?? "");
  // Enter and Esc unmount the field; its blur must not commit a second time.
  const settled = useRef(false);
  const finish = (commit: boolean) => {
    if (settled.current) return;
    settled.current = true;
    if (commit) onDone(draft);
    else onCancel();
  };
  return (
    <div className="flex items-center gap-2 bg-accent px-3 py-1.5">
      <BookmarkSimple size={12} weight="fill" className="shrink-0 text-brand" aria-hidden />
      <Input
        autoFocus
        value={draft}
        placeholder="Name this version"
        aria-label="Version name"
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => finish(true)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            finish(true);
          } else if (e.key === "Escape") {
            e.preventDefault();
            finish(false);
          }
        }}
        className="h-7 bg-card text-[12px]"
      />
    </div>
  );
}
