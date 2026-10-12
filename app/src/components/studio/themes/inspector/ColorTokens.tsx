import { ColorField } from "@/components/ui/color/ColorField";
import { Row, Section, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { sectionKeys, setToken, tokenAt, type Tokens } from "@/lib/studio/themeDraft";

/** Every `color.*` token, each with the colour control. */
export function ColorTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  const keys = sectionKeys(tokens, "color");
  return (
    <Section title="Colours" note="bg is the canvas under every frame; text colours any Text that sets none.">
      {keys.map((key) => {
        const value = tokenAt(tokens, ["color", key]);
        return (
          <Row key={key} label={tokenLabel(key)}>
            <ColorField
              label={tokenLabel(key)}
              value={typeof value === "string" ? value : ""}
              onChange={(hex) => onChange(setToken(tokens, ["color", key], hex))}
            />
          </Row>
        );
      })}
    </Section>
  );
}
