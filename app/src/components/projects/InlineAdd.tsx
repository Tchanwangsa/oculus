import { useEffect, useRef, useState } from "react";
import { Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/**
 * "Add one more of these", as a row that turns into a field.
 *
 * A project, a card, a subtask and a backlog stub are all one line of text at
 * the moment they are created — everything else about them is edited later, on
 * the thing itself — so none of them is worth a dialog. Enter commits and
 * leaves the field open for the next one, Escape and blur put it away.
 */
export function InlineAdd({
  label,
  placeholder,
  onAdd,
  defaultEditing = false,
  disabled,
  disabledReason,
  className,
  inputClassName,
}: {
  label: string;
  placeholder?: string;
  onAdd: (title: string) => void | Promise<void>;
  /** Start as the field rather than the button — for a composer opened by a
   *  control somewhere else on the row. */
  defaultEditing?: boolean;
  disabled?: boolean;
  /** Why the control is dead, e.g. subtasks being one level deep. */
  disabledReason?: string;
  className?: string;
  inputClassName?: string;
}) {
  const [editing, setEditing] = useState(defaultEditing);
  const [text, setText] = useState("");
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) ref.current?.focus();
  }, [editing]);

  const commit = () => {
    const title = text.trim();
    setText("");
    if (!title) {
      setEditing(false);
      return;
    }
    void onAdd(title);
  };

  if (editing && !disabled) {
    return (
      <input
        ref={ref}
        value={text}
        placeholder={placeholder ?? label}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => {
          commit();
          setEditing(false);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            setText("");
            setEditing(false);
          }
        }}
        className={cn(
          "w-full rounded-lg border border-brand/40 bg-card px-2.5 py-1.5 text-xs text-foreground outline-none placeholder:text-muted-foreground",
          inputClassName,
        )}
      />
    );
  }

  return (
    <button
      type="button"
      disabled={disabled}
      title={disabled ? disabledReason : label}
      onClick={() => setEditing(true)}
      className={cn(
        "flex w-full items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-left text-xs text-muted-foreground transition-colors",
        disabled
          ? "cursor-not-allowed opacity-40"
          : "cursor-pointer hover:bg-accent hover:text-foreground",
        className,
      )}
    >
      <Plus size={12} weight="bold" className="shrink-0" />
      <span className="truncate">{label}</span>
    </button>
  );
}
