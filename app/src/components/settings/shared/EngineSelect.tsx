import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

/** Backend engine catalogues use the same selection shape. */
export interface EngineOption {
  id: string;
  label: string;
  available: boolean;
  detail: string;
  unavailable_reason: string | null;
}

/** Engines come from the backend; the section owns switching policy. */
export function EngineSelect({ label, value, disabled, engines, onChange }: {
  label: string;
  value: string;
  disabled: boolean;
  engines: readonly EngineOption[];
  onChange: (engine: string) => void;
}) {
  return (
    <Select value={value} disabled={disabled} onValueChange={onChange}>
      <SelectTrigger aria-label={label} size="sm" className="h-7 w-48 text-xs">
        <SelectValue placeholder="—" />
      </SelectTrigger>
      <SelectContent>
        {engines.map((engine) => (
          <SelectItem key={engine.id} value={engine.id} disabled={!engine.available} className="text-xs">
            <span>{engine.label}</span>
            {!engine.available && <span className="text-[11px] text-muted-foreground">Unavailable</span>}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
