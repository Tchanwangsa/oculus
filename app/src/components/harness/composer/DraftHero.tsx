import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { CaretDown } from "@phosphor-icons/react";
import { SubjectOptionGlyph } from "@/components/harness/composer/SubjectPicker";
import { subjectOption, type SubjectOption } from "@/components/harness/composer/subjectOptions";
import type { Subject } from "@/lib/db";
import type { ChatScope } from "@/lib/harness/scope";
import { cn } from "@/lib/utils";

const HEADING = "text-center font-display text-[26px] font-semibold tracking-tight text-foreground";
const PILL =
  "inline-flex min-w-0 max-w-full items-center gap-2 rounded-full border border-border bg-card px-3 py-0.5 align-middle leading-tight shadow-xs";

/**
 * A new chat's headline, naming its subject in a pill: "What should we work
 * on in [MAST20004]?", or "[Choose a subject] to start" until one is chosen.
 * The pill opens the new-thread dialog (`onChoose`). When the one-line
 * sentence is wider than the column, the pill drops under "What should we
 * work on?". The decision measures a hidden copy of the line against this
 * box, both by their own rects, so page zoom and a split pane scale them
 * alike (`docs/ui.md`).
 */
export function DraftHero({
  subjects,
  scope,
  onChoose,
  className,
}: {
  subjects: readonly Subject[];
  /** null is unchosen. */
  scope: ChatScope | null;
  onChoose: () => void;
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

  const chosen = subjectOption(subjects, scope);
  const pill = cn(PILL, !chosen && "border-dashed text-brand");
  const title = chosen ? chosen.title : "Choose a subject, or General for the whole library";
  const picker = (
    <button
      type="button"
      title={title}
      aria-label={`Subject: ${title}`}
      aria-haspopup="dialog"
      onClick={onChoose}
      className={cn(
        pill,
        "cursor-pointer transition-[color,background-color,border-color] outline-none hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/25",
      )}
    >
      <PillFace option={chosen} />
    </button>
  );

  return (
    <div ref={boxRef} className={cn("relative w-full", className)}>
      <div aria-hidden className="pointer-events-none invisible absolute inset-x-0 top-0 h-0 overflow-hidden">
        <div ref={lineRef} className={cn(HEADING, "w-max whitespace-nowrap")}>
          <Sentence scope={scope} picker={<span className={pill}><PillFace option={chosen} /></span>} />
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

/** The pill's glyph, label and caret; drawn again in the hidden line that is measured. */
function PillFace({ option }: { option: SubjectOption | null }) {
  return (
    <>
      {option && <SubjectOptionGlyph option={option} size={22} />}
      <span className="min-w-0 truncate">{option ? option.label : "Choose a subject"}</span>
      <CaretDown size={16} weight="bold" className="shrink-0 text-muted-foreground" />
    </>
  );
}
