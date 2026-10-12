import { useEffect, useRef, useState } from "react";

import { Segmented } from "@/components/studio/style/Segmented";
import { LENGTHS } from "@/components/studio/style/style";
import { Input } from "@/components/ui/input";
import { lengthInput, parseLength } from "@/lib/studio/length";

const OPTIONS = [
  { value: "auto", label: "Auto" },
  ...LENGTHS.map((l) => ({ value: String(l.value), label: l.label })),
  { value: "custom", label: "Custom" },
];

/**
 * The length row: Auto (no target), the presets, or a typed length. A typed
 * length is committed as soon as it parses; while it does not, the last good
 * length stays and the sentence says why. Leaving the field puts the current
 * length back in it.
 */
export function LengthField({ value, onChange }: { value: number | null; onChange: (secs: number | null) => void }) {
  const preset = value !== null && LENGTHS.some((l) => l.value === value);
  const [custom, setCustom] = useState(value !== null && !preset);
  const [text, setText] = useState(value !== null && !preset ? lengthInput(value) : "");
  const [problem, setProblem] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (custom) input.current?.focus();
  }, [custom]);

  const pick = (key: string) => {
    setProblem(null);
    if (key === "custom") {
      setText(value === null ? "" : lengthInput(value));
      setCustom(true);
      return;
    }
    setCustom(false);
    onChange(key === "auto" ? null : Number(key));
  };

  const type = (next: string) => {
    setText(next);
    if (!next.trim()) {
      setProblem(null);
      return;
    }
    const parsed = parseLength(next);
    if ("problem" in parsed) {
      setProblem(parsed.problem);
      return;
    }
    setProblem(null);
    onChange(parsed.secs);
  };

  const leave = () => {
    setProblem(null);
    setText(value === null ? "" : lengthInput(value));
  };

  const selected = custom ? "custom" : value === null ? "auto" : String(value);
  return (
    <div className="space-y-1.5">
      <Segmented dense label="Length" options={OPTIONS} value={selected} onChange={pick} />
      {custom && (
        <div className="space-y-1">
          <Input
            ref={input}
            value={text}
            onChange={(e) => type(e.target.value)}
            onBlur={leave}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                e.currentTarget.blur();
              }
            }}
            inputMode="text"
            autoComplete="off"
            spellCheck={false}
            placeholder="45, 90s, 3m or 1:30"
            aria-label="Custom length"
            aria-invalid={problem !== null}
            className="h-7 text-[12px] tabular-nums"
          />
          {problem && <p className="px-0.5 text-[11px] text-destructive">{problem}</p>}
        </div>
      )}
    </div>
  );
}
