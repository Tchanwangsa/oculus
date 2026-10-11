import { useEffect, useMemo, useRef, useState } from "react";
import { Plus, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { allTags, normaliseTags } from "@/lib/planning/projects";

/**
 * A project's tags as removable pills. Hands back the whole set
 * (`updateProject` replaces `tags`); {@link normaliseTags} owns trimming,
 * dedup and caps on the way into the column.
 */
export function TagEditor({
  tags,
  onChange,
}: {
  tags: string[];
  onChange: (tags: string[]) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [text, setText] = useState("");
  /** Every tag used anywhere; `null` until the field first opens. */
  const [vocab, setVocab] = useState<string[] | null>(null);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (adding) ref.current?.focus();
  }, [adding]);

  // Re-read on each opening so tags added elsewhere since mount are offered.
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
              // Enter and comma both add and keep the field open.
              if (e.key === "Enter") {
                e.preventDefault();
                add(text);
              }
              if (e.key === ",") {
                e.preventDefault();
                add(text);
              }
              if (e.key === "Escape") {
                setText("");
                setAdding(false);
              }
            }}
            // Blur must not commit: a clicked suggestion blurs the input first,
            // and would add the half-typed text too.
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
              // On mousedown with preventDefault: the input's blur would
              // unmount this row before a click landed.
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

const MAX_SUGGESTIONS = 6;
