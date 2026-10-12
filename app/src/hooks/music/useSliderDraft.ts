import { useEffect, useState } from "react";

/** How long a committed value outlives a snapshot that never confirms it. */
const CONFIRM_MS = 2000;

/**
 * A slider's own value while it is dragged and until Rust's next snapshot
 * confirms it (`confirmedBy` changes), so the thumb doesn't jump back to the
 * old value between the release and the answer. Null when the snapshot's
 * value should show.
 */
export function useSliderDraft(confirmedBy: unknown) {
  const [draft, setDraft] = useState<{ value: number; done: boolean } | null>(null);

  useEffect(() => {
    setDraft((d) => (d?.done ? null : d));
  }, [confirmedBy]);

  useEffect(() => {
    if (!draft?.done) return;
    const timer = window.setTimeout(() => setDraft(null), CONFIRM_MS);
    return () => window.clearTimeout(timer);
  }, [draft]);

  return {
    draft: draft?.value ?? null,
    change: (value: number) => setDraft({ value, done: false }),
    commit: (value: number) => setDraft({ value, done: true }),
  };
}
