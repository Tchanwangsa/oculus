import { Note } from "@/components/studio/themes/inspector/Fields";
import { InlineField, NumberInput } from "@/components/studio/panel/fields";
import { setToken, tokenAt, type Tokens } from "@/lib/studio/themeDraft";

const NARRATION: ReadonlyArray<{ key: string; label: string; min: number }> = [
  { key: "lead", label: "Before the voice", min: 0 },
  { key: "tail", label: "After the voice", min: 0 },
  { key: "pause", label: "A [pause]", min: 0.05 },
];

/** The narration's silences (`narration.*`, seconds). */
export function NarrationTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  return (
    <>
      <div className="space-y-1">
        {NARRATION.map(({ key, label, min }) => (
          <InlineField key={key} label={label}>
            <NumberInput
              label={label}
              value={tokenAt(tokens, ["narration", key]) as number | undefined}
              min={min}
              step={0.05}
              unit="s"
              onChange={(n) => onChange(setToken(tokens, ["narration", key], n))}
            />
          </InlineField>
        ))}
      </div>
      <Note>Silence in a narrated scene. A change re-times every narrated video in this theme.</Note>
    </>
  );
}
