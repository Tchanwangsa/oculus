import { PillTabs } from "@/components/ui/table/PillTabs";

/** The component's named examples, one chosen. */
export function ExamplePills({
  names,
  value,
  onChange,
}: {
  names: string[];
  value: string | null;
  onChange: (name: string) => void;
}) {
  if (names.length === 0) return null;
  return (
    <div className="flex items-center gap-2">
      <span className="text-[11px] font-medium tracking-wide text-muted-foreground">Example</span>
      <PillTabs
        tabs={names.map((n) => ({ value: n, label: n }))}
        value={value ?? names[0]}
        onChange={onChange}
        className="flex-wrap gap-1"
      />
    </div>
  );
}
