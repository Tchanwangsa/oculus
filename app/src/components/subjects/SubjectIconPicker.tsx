import { useMemo, useState } from "react";
import { MagnifyingGlass } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ICON_CATALOG } from "@/components/subjects/iconCatalog";
import {
  courseColor,
  ICON_COLORS,
  iconKey,
  useSubjectIconStore,
} from "@/stores/subjectIconStore";

/**
 * Notion-style icon picker for a subject: colour swatches on top, a filterable
 * Phosphor grid below. Every choice applies immediately; Reset returns the
 * subject to the default Book in its hashed colour.
 */
export function SubjectIconPicker({
  code,
  children,
}: {
  code: string;
  children: React.ReactNode;
}) {
  const pref = useSubjectIconStore((s) => s.prefs[iconKey(code)]);
  const setPref = useSubjectIconStore((s) => s.setPref);
  const clearPref = useSubjectIconStore((s) => s.clearPref);
  const [query, setQuery] = useState("");

  const activeColor = pref?.color ?? courseColor(code);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return ICON_CATALOG;
    return ICON_CATALOG.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        c.keywords.some((k) => k.includes(q)),
    );
  }, [query]);

  return (
    <Popover onOpenChange={(open) => !open && setQuery("")}>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent align="start" sideOffset={8} className="w-[19rem] p-0">
        <div className="flex items-center gap-2 border-b border-border-subtle px-3 py-2">
          <div className="flex flex-1 items-center gap-1.5 rounded-md bg-surface px-2 py-1">
            <MagnifyingGlass size={12} className="shrink-0 text-muted-foreground" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Filter…"
              autoFocus
              className="w-full bg-transparent text-[12px] text-foreground placeholder:text-muted-foreground outline-none"
            />
          </div>
          {pref && (
            <button
              type="button"
              onClick={() => clearPref(code)}
              className="shrink-0 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
            >
              Reset
            </button>
          )}
        </div>

        <div className="flex items-center gap-1.5 px-3 py-2.5">
          {/* Auto = the hashed course colour; a stored colour overrides it. */}
          <ColorSwatch
            color={courseColor(code)}
            label="Auto"
            selected={pref?.color == null}
            onClick={() => setPref(code, { color: undefined })}
          />
          <div className="mx-0.5 h-4 w-px bg-border-subtle" />
          {ICON_COLORS.map((c) => (
            <ColorSwatch
              key={c.value}
              color={c.value}
              label={c.name}
              selected={pref?.color === c.value}
              onClick={() => setPref(code, { color: c.value })}
            />
          ))}
        </div>

        <div className="grid max-h-52 grid-cols-8 gap-0.5 overflow-y-auto px-2 pb-2">
          {shown.map(({ name, Icon }) => (
            <button
              key={name}
              type="button"
              title={name}
              onClick={() => setPref(code, { icon: name })}
              className={cn(
                "flex h-8 w-8 items-center justify-center rounded-md transition-colors hover:bg-surface",
                pref?.icon === name && "bg-surface-raised",
              )}
            >
              <Icon size={17} weight="fill" color={activeColor} />
            </button>
          ))}
          {shown.length === 0 && (
            <p className="col-span-8 py-6 text-center text-[12px] text-muted-foreground">
              No icons match
            </p>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

function ColorSwatch({
  color,
  label,
  selected,
  onClick,
}: {
  color: string;
  label: string;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        "h-[18px] w-[18px] rounded-full transition-shadow",
        selected
          ? "ring-2 ring-primary ring-offset-2 ring-offset-popover"
          : "hover:ring-2 hover:ring-border hover:ring-offset-2 hover:ring-offset-popover",
      )}
      style={{ backgroundColor: color }}
    />
  );
}
