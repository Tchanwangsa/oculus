import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { cn } from "@/lib/utils";

/** Document title editing, shared by tasks and projects. */
export function DetailTitle({ title, label, done = false, href, onRename }: {
  title: string;
  label: string;
  done?: boolean;
  href: (title: string) => string;
  onRename: (title: string) => void;
}) {
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(title);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) ref.current?.select();
  }, [editing]);

  const commit = () => {
    setEditing(false);
    const next = draft.trim();
    if (!next || next === title) {
      setDraft(title);
      return;
    }
    onRename(next);
    // The tab strip reads the title from the href, so re-navigate;
    // replace keeps the back arrow where it was.
    navigate(href(next), { replace: true });
  };

  const edit = () => {
    setDraft(title);
    setEditing(true);
  };

  const shared =
    "w-full font-display text-[30px] leading-tight font-bold tracking-tight break-words outline-none";

  if (editing) {
    return (
      <input
        ref={ref}
        aria-label={label}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            setDraft(title);
            setEditing(false);
          }
        }}
        className={cn(shared, "rounded-md bg-transparent text-foreground")}
      />
    );
  }

  return (
    <h1
      aria-label={label}
      tabIndex={0}
      onClick={edit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          edit();
        }
      }}
      className={cn(
        shared,
        "cursor-text rounded-md",
        done ? "text-muted-foreground line-through" : "text-foreground",
      )}
    >
      {title}
    </h1>
  );
}
