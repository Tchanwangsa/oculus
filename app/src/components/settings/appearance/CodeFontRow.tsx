import { useEffect, useState } from "react";

import { Input } from "@/components/ui/input";
import { setAppearance, useAppearance } from "@/lib/ui/themes";
import { AppearanceRow } from "./AppearanceRow";

/** The code font, a draft until the field is left or Enter is pressed. */
export function CodeFontRow() {
  const { codeFont } = useAppearance();
  const [draft, setDraft] = useState(codeFont);
  // A rejected name (quotes, braces) comes back empty; show what is stored.
  useEffect(() => setDraft(codeFont), [codeFont]);

  const commit = () => {
    if (draft.trim() !== codeFont) setAppearance({ codeFont: draft });
    else setDraft(codeFont);
  };

  return (
    <AppearanceRow
      label="Code font"
      description="Code blocks and terminal output. Any monospace font installed on this machine."
    >
      <Input
        aria-label="Code font"
        value={draft}
        placeholder="e.g. JetBrains Mono"
        spellCheck={false}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
        }}
        className="h-7 w-56 font-mono text-xs"
      />
    </AppearanceRow>
  );
}
