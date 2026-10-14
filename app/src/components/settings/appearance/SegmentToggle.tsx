import type { Icon } from "@phosphor-icons/react";

import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  icon?: Icon;
}

/** A small segmented switch: one of a few values, the chosen one raised. */
export function SegmentToggle<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly SegmentOption<T>[];
  onChange: (value: T) => void;
}) {
  return (
    <ToggleGroup
      type="single"
      size="sm"
      aria-label={label}
      value={value}
      onValueChange={(v) => v && onChange(v as T)}
      className="max-w-full flex-wrap bg-surface rounded-md p-0.5"
    >
      {options.map(({ value: option, label: name, icon: Glyph }) => (
        <ToggleGroupItem
          key={option}
          value={option}
          aria-label={name}
          className="h-6 gap-1.5 px-2.5 text-xs rounded-[5px] data-[state=on]:bg-card data-[state=on]:shadow-xs"
        >
          {Glyph && <Glyph size={12} weight={value === option ? "fill" : "regular"} />}
          {name}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}
