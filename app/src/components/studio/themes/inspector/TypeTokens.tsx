import { NameSelect, NumberField, Row, Section, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { THEME_FONTS, sectionKeys, setFontRole, setToken, tokenAt, type FontRole, type Tokens } from "@/lib/studio/themeDraft";

const ROLES: ReadonlyArray<{ role: FontRole; label: string }> = [
  { role: "heading", label: "Headings" },
  { role: "body", label: "Body" },
  { role: "mono", label: "Code" },
];

/** The three font families (a change moves the type sizes that used the old
 *  family with it) and each type style's size in px. */
export function TypeTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  const sizes = sectionKeys(tokens, "type").filter((k) => typeof tokenAt(tokens, ["type", k, "fontSize"]) === "number");
  return (
    <>
      <Section title="Fonts" note="Only the fonts built into Studio, so a frame looks the same on every machine.">
        {ROLES.map(({ role, label }) => {
          const family = tokenAt(tokens, ["font", role]);
          return (
            <Row key={role} label={label}>
              <NameSelect
                label={`${label} font`}
                value={typeof family === "string" ? family : undefined}
                options={THEME_FONTS}
                onChange={(f) => onChange(setFontRole(tokens, role, f))}
              />
            </Row>
          );
        })}
      </Section>
      <Section title="Type sizes">
        {sizes.map((key) => (
          <Row key={key} label={tokenLabel(key)}>
            <NumberField
              label={`${tokenLabel(key)} size`}
              value={tokenAt(tokens, ["type", key, "fontSize"]) as number}
              min={1}
              unit="px"
              onChange={(n) => onChange(setToken(tokens, ["type", key, "fontSize"], n))}
            />
          </Row>
        ))}
      </Section>
    </>
  );
}
