import { useEffect, useMemo, useRef, useState } from "react";
import { Plus, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { allTags, normaliseTags } from "@/lib/projects";

/**
 * A project's own labels, as pills you can add to and take away from.
 *
 * The whole set is written at once — `updateProject` replaces `tags` and has
 * no add/remove patch — so this component hands back the list it is showing
 * rather than a delta, and never needs to know what it is merging into.
 *
 * Nothing here trims, deduplicates or caps: {@link normaliseTags} does all
 * three on the way into the column, and a second copy of those rules is a
 * second place for `Draft` and `draft` to start disagreeing. It *is* called
 * here, on the one thing the column cannot decide for us — whether what has
 * just been typed is a tag this project already carries.
 */
export function TagEditor({
  tags,
  onChange,
}: {
  tags: string[];
  /** The replacement set, in display order. */
  onChange: (tags: string[]) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [text, setText] = useState("");
  /** Every tag used anywhere, for the suggestions. `null` until the field has
   *  been opened at least once — see the effect below. */
  const [vocab, setVocab] = useState<string[] | null>(null);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (adding) ref.current?.focus();
  }, [adding]);

  // Read when the field opens, not per keystroke: `allTags` scans every
  // project's `tags` column, and filtering the result is a substring test over
  // a handful of strings. Re-read on each opening rather than once for the
  // life of the component, so a tag invented on another project since the page
  // was mounted is offered here too.
  useEffect(() => {
    if (!adding) return;
    let cancelled = false;
    allTags()
      .then((all) => !cancelled && setVocab(all))
      .catch((e) => {
        console.error("read tag vocabulary failed", e);
        if (!cancelled) setVocab([]);
      });
    return () => {
      cancelled = true;
    };
  }, [adding]);

  const add = (raw: string) => {
    // `normaliseTags` for the trim and the whitespace collapse, so "  final
    // draft " and "final  draft" are the same typed tag here and in the column.
    const [tag] = normaliseTags([raw]);
    setText("");
    if (!tag) return;
    if (tags.some((t) => t.toLowerCase() === tag.toLowerCase())) return;
    onChange([...tags, tag]);
  };

  const suggestions = useMemo(() => {
    if (!vocab) return [];
    const typed = text.trim().toLowerCase();
    const have = new Set(tags.map((t) => t.toLowerCase()));
    return vocab
      .filter((t) => !have.has(t.toLowerCase()) && t.toLowerCase().includes(typed))
      .slice(0, MAX_SUGGESTIONS);
  }, [vocab, text, tags]);

  return (
    <div className="flex w-full min-w-0 flex-col gap-1.5">
      <div className="flex flex-wrap items-center gap-1.5">
        {tags.map((tag) => (
          <span
            key={tag}
            className="inline-flex max-w-full items-center gap-1 rounded-full bg-secondary px-2 py-0.5 text-[11px] text-foreground"
          >
            <span className="min-w-0 truncate">{tag}</span>
            <button
              type="button"
              aria-label={`Remove ${tag}`}
              onClick={() => onChange(tags.filter((t) => t !== tag))}
              className="shrink-0 cursor-pointer text-muted-foreground transition-colors hover:text-foreground"
            >
              <X size={9} weight="bold" />
            </button>
          </span>
        ))}

        {adding ? (
          <input
            ref={ref}
            value={text}
            placeholder="Tag"
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              // Enter adds and leaves the field open for the next one, the way
              // `InlineAdd` does — tags arrive in twos and threes.
              if (e.key === "Enter") {
                e.preventDefault();
                add(text);
              }
              // A comma is what people type *between* tags, so it means the
              // same thing as Enter rather than ending up inside a label.
              if (e.key === ",") {
                e.preventDefault();
                add(text);
              }
              if (e.key === "Escape") {
                setText("");
                setAdding(false);
              }
            }}
            // Blur closes without committing, which is where this parts company
            // with `InlineAdd`: the suggestions below are clicked, and a blur
            // that committed the half-typed text would add both it and the
            // suggestion that was clicked.
            onBlur={() => {
              setText("");
              setAdding(false);
            }}
            className={cn(
              "w-24 rounded-full border border-brand/40 bg-card px-2 py-0.5 text-[11px] text-foreground outline-none",
              "placeholder:text-muted-foreground/60",
            )}
          />
        ) : (
          <button
            type="button"
            aria-label="Add a tag"
            onClick={() => setAdding(true)}
            className="inline-flex cursor-pointer items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
          >
            <Plus size={9} weight="bold" className="shrink-0" />
            {tags.length === 0 && "Add a tag"}
          </button>
        )}
      </div>

      {adding && suggestions.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          {suggestions.map((tag) => (
            <button
              key={tag}
              type="button"
              // The input's blur fires before a click lands, and closing the
              // field unmounts this row mid-gesture — so the press is taken on
              // mousedown and the focus is never given up in the first place.
              onMouseDown={(e) => {
                e.preventDefault();
                add(tag);
              }}
              className="cursor-pointer rounded-full border border-dashed border-border px-2 py-0.5 text-[11px] text-muted-foreground transition-colors hover:border-solid hover:text-foreground"
            >
              {tag}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Six. The suggestions sit inside a property row, and a list long enough to
 *  need scrolling is a list you would have been faster typing. */
const MAX_SUGGESTIONS = 6;
