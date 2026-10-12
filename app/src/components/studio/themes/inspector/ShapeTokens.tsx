import { NumberField, Row, Section, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { sectionKeys, setToken, tokenAt, type Tokens } from "@/lib/studio/themeDraft";

/** The corner radii (`radius.*`, px) and the narration's silences
 *  (`narration.*`, seconds). */
export function CornerTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  return (
    <Section title="Corners">
      {sectionKeys(tokens, "radius").map((key) => (
        <Row key={key} label={tokenLabel(key)}>
          <NumberField
            label={`${tokenLabel(key)} radius`}
            value={tokenAt(tokens, ["radius", key]) as number}
            unit="px"
            onChange={(n) => onChange(setToken(tokens, ["radius", key], n))}
          />
        </Row>
      ))}
    </Section>
  );
}

const NARRATION: ReadonlyArray<{ key: string; label: string; min: number }> = [
  { key: "lead", label: "Before the voice", min: 0 },
  { key: "tail", label: "After the voice", min: 0 },
  { key: "pause", label: "A [pause]", min: 0.05 },
];

export function NarrationTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  return (
    <Section title="Narration" note="Silence in a narrated scene. A change re-times every narrated video in this theme.">
      {NARRATION.map(({ key, label, min }) => (
        <Row key={key} label={label}>
          <NumberField
            label={label}
            value={tokenAt(tokens, ["narration", key]) as number | undefined}
            min={min}
            step={0.05}
            unit="s"
            onChange={(n) => onChange(setToken(tokens, ["narration", key], n))}
          />
        </Row>
      ))}
    </Section>
  );
}
