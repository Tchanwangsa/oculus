import { useMemo, useRef, useState } from "react";
import { MagnifyingGlass } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { ICON_CATALOG } from "./iconCatalog";
import { ICON_COLORS } from "@/lib/ui/iconPreferences";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { IconPickerProps } from "./IconPicker";

/** The same searchable catalogue and palette for subjects and projects. */
export default function IconPickerBody({ value, defaultColor, defaultName, weight = "fill", onChange, onReset }: Omit<IconPickerProps, "children">) {
  const [query, setQuery] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "y", query);
  const activeColor = value?.color ?? defaultColor;
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
    <>
      <div className="flex items-center gap-2 border-b border-border-subtle px-3 py-2">
        <div className="flex flex-1 items-center gap-1.5 rounded-md bg-surface px-2 py-1">
          <MagnifyingGlass size={12} className="shrink-0 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search icons…"
            aria-label="Search icons"
            autoFocus
            className="w-full bg-transparent text-[12px] text-foreground placeholder:text-muted-foreground outline-none"
          />
        </div>
        {value && (
          <button
            type="button"
            onClick={onReset}
            className="shrink-0 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
          >
            Reset
          </button>
        )}
      </div>

      <div className="flex items-center gap-1.5 px-3 py-2.5">
        {/* Auto follows the subject colour (or grey for Personal projects). */}
        <ColorSwatch
          color={defaultColor}
          label="Auto"
          selected={value?.color == null}
          onClick={() => onChange({ color: undefined })}
        />
        <div className="mx-0.5 h-4 w-px bg-border-subtle" />
        {ICON_COLORS.map((c) => (
          <ColorSwatch
            key={c.value}
            color={c.value}
            label={c.name}
            selected={value?.color === c.value}
            onClick={() => onChange({ color: c.value })}
          />
        ))}
      </div>

      <div ref={ref} className="grid max-h-52 grid-cols-8 gap-0.5 overflow-y-auto px-2 pb-2">
        {shown.map(({ name, Icon }) => (
          <button
            key={name}
            type="button"
            title={name}
            aria-label={`Use ${name} icon`}
            aria-pressed={(value?.icon ?? defaultName) === name}
            onClick={() => onChange({ icon: name })}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-md transition-colors hover:bg-surface",
              (value?.icon ?? defaultName) === name && "bg-surface-raised",
            )}
          >
            <Icon size={17} weight={weight} color={activeColor} />
          </button>
        ))}
        {shown.length === 0 && (
          <p className="col-span-8 py-6 text-center text-[12px] text-muted-foreground">
            No icons match
          </p>
        )}
      </div>
    </>
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
          ? "ring-2 ring-brand ring-offset-2 ring-offset-popover"
          : "hover:ring-2 hover:ring-border hover:ring-offset-2 hover:ring-offset-popover",
      )}
      style={{ backgroundColor: color }}
    />
  );
}
