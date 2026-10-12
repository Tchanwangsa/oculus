import { useState } from "react";

import { Segmented } from "@/components/studio/style/Segmented";
import { FieldGrid, Note, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { LabeledField, NumberInput, SubLabel } from "@/components/studio/panel/fields";
import { sectionKeys, setToken, tokenAt, type Tokens } from "@/lib/studio/themeDraft";

const SPRING_PARTS = ["stiffness", "damping", "mass"] as const;
const BEZIER = ["x1", "y1", "x2", "y2"] as const;

/** Durations, easings and the spring presets (`motion.duration`,
 *  `motion.easing`, `motion.spring`). */
export function MotionTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  const set = (path: string[], value: unknown) => onChange(setToken(tokens, ["motion", ...path], value));
  const durations = sectionKeys(tokens, "motion.duration");
  const easings = sectionKeys(tokens, "motion.easing");
  const springs = sectionKeys(tokens, "motion.spring");
  const [spring, setSpring] = useState<string | null>(null);
  const shownSpring = spring && springs.includes(spring) ? spring : (springs[0] ?? null);

  return (
    <>
      {durations.length > 0 && (
        <>
          <SubLabel>Durations</SubLabel>
          <FieldGrid>
            {durations.map((key) => (
              <LabeledField key={key} label={tokenLabel(key)}>
                <NumberInput
                  label={`${tokenLabel(key)} duration`}
                  value={tokenAt(tokens, ["motion", "duration", key]) as number}
                  min={0}
                  step={0.05}
                  unit="s"
                  onChange={(n) => set(["duration", key], n)}
                />
              </LabeledField>
            ))}
          </FieldGrid>
        </>
      )}
      {easings.length > 0 && (
        <>
          <SubLabel className="pt-3">Easings</SubLabel>
          <div className="space-y-2.5">
            {easings.map((key) => {
              const points = tokenAt(tokens, ["motion", "easing", key]);
              const at = Array.isArray(points) ? points : [];
              return (
                <LabeledField key={key} label={tokenLabel(key)}>
                  <FieldGrid cols={4} className="gap-x-1">
                    {BEZIER.map((p, i) => (
                      <NumberInput
                        key={p}
                        label={`${tokenLabel(key)} ${p}`}
                        prefix={p}
                        value={typeof at[i] === "number" ? (at[i] as number) : undefined}
                        min={-10}
                        step={0.05}
                        onChange={(n) => set(["easing", key], BEZIER.map((_, j) => (j === i ? n : (at[j] ?? 0))))}
                      />
                    ))}
                  </FieldGrid>
                </LabeledField>
              );
            })}
          </div>
          <Note>Cubic-bezier points, as in CSS.</Note>
        </>
      )}
      {shownSpring && (
        <>
          <SubLabel className="pt-3">Springs</SubLabel>
          <Segmented label="Spring preset" options={springs.map((n) => ({ value: n, label: tokenLabel(n) }))} value={shownSpring} onChange={setSpring} />
          <FieldGrid cols={3} className="pt-2.5">
            {SPRING_PARTS.map((part) => (
              <LabeledField key={part} label={tokenLabel(part)}>
                <NumberInput
                  label={`${shownSpring} ${part}`}
                  value={tokenAt(tokens, ["motion", "spring", shownSpring, part]) as number}
                  min={part === "damping" ? 0 : 0.01}
                  step={part === "mass" ? 0.1 : 1}
                  onChange={(n) => set(["spring", shownSpring, part], n)}
                />
              </LabeledField>
            ))}
          </FieldGrid>
          <Note>Components name a preset (gentle, snappy, bouncy) for things that pop in.</Note>
        </>
      )}
    </>
  );
}
