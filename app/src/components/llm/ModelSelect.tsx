import { useMemo } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { modelKey, type LlmProvider, type ModelRef } from "@/lib/db";
import { cn } from "@/lib/utils";

/** Pick one model out of the library. Deliberately not searchable: the
 *  library is the short list the user curated, and the searching happens where
 *  the hundreds are — the model browser. */
export function ModelSelect({
  library,
  providers,
  value,
  onChange,
  placeholder = "Choose a model",
  className,
  disabled,
}: {
  library: ModelRef[];
  providers: LlmProvider[];
  value: ModelRef | null;
  onChange: (m: ModelRef) => void;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
}) {
  const byKey = useMemo(
    () => new Map(library.map((m) => [modelKey(m), m])),
    [library],
  );
  const providerLabel = (id: string) =>
    providers.find((p) => p.id === id)?.label ?? "missing provider";

  return (
    <Select
      value={value ? modelKey(value) : ""}
      onValueChange={(k) => {
        const m = byKey.get(k);
        if (m) onChange(m);
      }}
      disabled={disabled || library.length === 0}
    >
      <SelectTrigger size="sm" className={cn("h-7 text-xs", className)}>
        <SelectValue
          placeholder={library.length ? placeholder : "No models in your library"}
        />
      </SelectTrigger>
      <SelectContent className="max-h-72">
        {library.map((m) => (
          <SelectItem key={modelKey(m)} value={modelKey(m)} className="text-xs">
            <span className="truncate">{m.model}</span>
            <span className="text-muted-foreground/70">
              {providerLabel(m.providerId)}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
