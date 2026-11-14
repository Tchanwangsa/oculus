import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { SubjectPicker, SubjectPickerFace } from "@/components/harness/composer/SubjectPicker";
import type { Subject } from "@/lib/db";
import type { ChatScope } from "@/lib/harness/scope";
import { cn } from "@/lib/utils";

const HEADING = "text-center font-display text-[26px] font-semibold tracking-tight text-foreground";

/**
 * A new chat's headline, holding its subject picker: "What should we work on
 * in [MAST20004]?", or "[Choose a subject] to start" until one is chosen.
 * When the one-line sentence is wider than the column, the picker drops
 * under "What should we work on?". The decision measures a hidden copy of
 * the line against this box, both by their own rects, so page zoom and a
 * split pane scale them alike (`docs/ui.md`).
 */
export function DraftHero({
  subjects,
  scope,
  onScope,
  className,
}: {
  subjects: readonly Subject[];
  /** null is unchosen. */
  scope: ChatScope | null;
  onScope: (scope: ChatScope) => void;
  className?: string;
}) {
  const boxRef = useRef<HTMLDivElement>(null);
  const lineRef = useRef<HTMLDivElement>(null);
  const [stacked, setStacked] = useState(false);

  useLayoutEffect(() => {
    const box = boxRef.current;
    const line = lineRef.current;
    if (!box || !line) return;
    // Half a pixel of slack: a sentence that exactly fits must not flip.
    const measure = () =>
      setStacked(line.getBoundingClientRect().width > box.getBoundingClientRect().width + 0.5);
    measure();
    // The line changes width with the subject and when the font loads.
    const observer = new ResizeObserver(measure);
    observer.observe(box);
    observer.observe(line);
    return () => observer.disconnect();
  }, []);

  const picker = <SubjectPicker variant="heading" subjects={subjects} value={scope} onChange={onScope} />;

  return (
    <div ref={boxRef} className={cn("relative w-full", className)}>
      <div aria-hidden className="pointer-events-none invisible absolute inset-x-0 top-0 h-0 overflow-hidden">
        <div ref={lineRef} className={cn(HEADING, "w-max whitespace-nowrap")}>
          <Sentence scope={scope} picker={<SubjectPickerFace variant="heading" subjects={subjects} value={scope} />} />
        </div>
      </div>
      {stacked ? (
        <h2 className={cn(HEADING, "flex flex-col items-center gap-2")}>
          {scope == null ? (
            <>
              {picker}
              <span>to start</span>
            </>
          ) : (
            <>
              <span>What should we work on?</span>
              {picker}
            </>
          )}
        </h2>
      ) : (
        <h2 className={HEADING}>
          <Sentence scope={scope} picker={picker} />
        </h2>
      )}
    </div>
  );
}

function Sentence({ scope, picker }: { scope: ChatScope | null; picker: ReactNode }) {
  return scope == null ? (
    <>
      {picker} to start
    </>
  ) : (
    <>
      What should we work on in {picker}?
    </>
  );
}
