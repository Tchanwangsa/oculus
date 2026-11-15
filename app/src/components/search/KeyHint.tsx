/** A key cap and what it does, for the footer of a palette. */
export function KeyHint({ keys, label }: { keys: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <kbd className="rounded border border-border bg-surface px-1 py-0.5 text-[10px] leading-none text-muted-foreground">
        {keys}
      </kbd>
      {label}
    </span>
  );
}
