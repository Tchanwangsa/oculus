import { useState } from "react";
import { CaretRight } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** A settled request as one work-row line (`RowShell`'s look): how it was
 *  settled and what it was about, its notes always shown under it, the
 *  whole request when opened. */
export function SettledRow({
  icon: IconC,
  state,
  label,
  subject,
  title,
  notes,
  warn,
  children,
}: {
  icon: Icon;
  /** `data-request-state`: how it was settled. */
  state: string;
  label: string;
  subject: string;
  title: string;
  notes: string[];
  warn: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div data-request-state={state} className="min-w-0">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className={cn(
          "group/row flex w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-md px-2 py-0.5 text-left text-xs leading-5 transition-colors hover:text-foreground",
          open ? "text-foreground" : "text-muted-foreground",
        )}
      >
        <IconC size={14} className="shrink-0" aria-hidden />
        <span className="shrink-0">{label}</span>
        <span className="min-w-0 truncate font-medium text-foreground/80" title={subject}>{subject}</span>
        <CaretRight
          size={11}
          className={cn(
            "ml-auto shrink-0 transition-[opacity,transform] will-change-[opacity,transform] duration-150",
            open ? "rotate-90 opacity-60" : "opacity-0 group-hover/row:opacity-60",
          )}
        />
      </button>
      {notes.length > 0 && (
        <div className="pl-[30px] pr-2">
          <RequestNotes notes={notes} warn={warn} />
        </div>
      )}
      {open && (
        <div className="flex flex-col gap-1.5 pb-1 pl-[30px] pr-2 pt-0.5">
          <p className="text-[11px] text-muted-foreground">{title}</p>
          {children}
        </div>
      )}
    </div>
  );
}

/** Rust's reason and what delivery knows, under a request; warning-toned
 *  when the answer may not have arrived. Null with nothing to say. */
export function RequestNotes({ notes, warn }: { notes: string[]; warn: boolean }) {
  if (notes.length === 0) return null;
  return (
    <div className="flex flex-col gap-0.5">
      {notes.map((note) => (
        <p
          key={note}
          className={cn("break-words text-[11px] leading-relaxed", warn ? "text-warning" : "text-muted-foreground")}
        >
          {note}
        </p>
      ))}
    </div>
  );
}
