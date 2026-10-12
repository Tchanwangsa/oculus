import { FieldGrid, Note, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { InlineField, LabeledField, NameSelect, NumberInput, SubLabel } from "@/components/studio/panel/fields";
import { THEME_FONTS, setFontRole, setToken, tokenAt, type FontRole, type Tokens } from "@/lib/studio/themeDraft";

const ROLES: ReadonlyArray<{ role: FontRole; label: string }> = [
  { role: "heading", label: "Headings" },
  { role: "body", label: "Body" },
  { role: "mono", label: "Code" },
];

/** The three font families (a change moves the type sizes that used the old
 *  family with it) and each type style's size in px. */
export function TypeTokens({
  tokens,
  sizes,
  onChange,
}: {
  tokens: Tokens;
  /** The `type.*` entries that have a size. */
  sizes: string[];
  onChange: (next: Tokens) => void;
}) {
  return (
    <>
      <SubLabel>Fonts</SubLabel>
      <div className="space-y-1">
        {ROLES.map(({ role, label }) => {
          const family = tokenAt(tokens, ["font", role]);
          return (
            <InlineField key={role} label={label}>
              <NameSelect
                label={`${label} font`}
                value={typeof family === "string" ? family : undefined}
                options={THEME_FONTS}
                onChange={(f) => onChange(setFontRole(tokens, role, f))}
              />
            </InlineField>
          );
        })}
      </div>
      <Note>Only the fonts built into Studio, so a frame looks the same on every machine.</Note>
      {sizes.length > 0 && (
        <>
          <SubLabel className="pt-3">Sizes</SubLabel>
          <FieldGrid>
            {sizes.map((key) => (
              <LabeledField key={key} label={tokenLabel(key)}>
                <NumberInput
                  label={`${tokenLabel(key)} size`}
                  value={tokenAt(tokens, ["type", key, "fontSize"]) as number}
                  min={1}
                  unit="px"
                  onChange={(n) => onChange(setToken(tokens, ["type", key, "fontSize"], n))}
                />
              </LabeledField>
            ))}
          </FieldGrid>
        </>
      )}
    </>
  );
}
