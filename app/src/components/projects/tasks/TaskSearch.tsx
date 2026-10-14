import { MagnifyingGlass, X } from "@phosphor-icons/react";

/** The pill search field above the tasks. Escape clears it; ⌘F is the
 *  shell's find, so there is no hint of it here. */
export function TaskSearch({
  value,
  onChange,
  placeholder = "Search tasks",
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  return (
    <div className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25 sm:max-w-72">
      <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && value) {
            e.preventDefault();
            onChange("");
          }
        }}
        placeholder={placeholder}
        aria-label={placeholder}
        spellCheck={false}
        autoComplete="off"
        className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
      />
      {value && (
        <button
          type="button"
          aria-label="Clear search"
          onClick={() => onChange("")}
          className="-mr-1 flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <X size={11} weight="bold" />
        </button>
      )}
    </div>
  );
}
