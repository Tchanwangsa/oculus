import { FieldGrid, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { LabeledField, NumberInput } from "@/components/studio/panel/fields";
import { sectionKeys, setToken, tokenAt, type Tokens } from "@/lib/studio/themeDraft";

/** The corner radii (`radius.*`, px). */
export function ShapeTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  return (
    <FieldGrid>
      {sectionKeys(tokens, "radius").map((key) => (
        <LabeledField key={key} label={tokenLabel(key)}>
          <NumberInput
            label={`${tokenLabel(key)} radius`}
            value={tokenAt(tokens, ["radius", key]) as number}
            min={0}
            unit="px"
            onChange={(n) => onChange(setToken(tokens, ["radius", key], n))}
          />
        </LabeledField>
      ))}
    </FieldGrid>
  );
}
