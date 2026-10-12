import { useState } from "react";

import { Segmented } from "@/components/studio/style/Segmented";
import { NameSelect, NumberField, Row, Section, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import {
  DIRECTIONS,
  MAGIC_MOVE,
  MORPH_COLOR,
  MOTION_BLUR,
  TRANSITION_KINDS,
  sectionKeys,
  setToken,
  tokenAt,
  type Tokens,
} from "@/lib/studio/themeDraft";

const SPRING_PARTS = ["stiffness", "damping", "mass"] as const;
const BEZIER = ["x1", "y1", "x2", "y2"] as const;

const BLUR_LABEL: Record<(typeof MOTION_BLUR)[number], string> = { finals: "Final renders", off: "Off", always: "Always" };
const BLUR_OPTIONS = MOTION_BLUR.map((v) => ({ value: v as string, label: BLUR_LABEL[v] }));

const options = (names: readonly string[]) => names.map((n) => ({ value: n, label: tokenLabel(n) }));

/** Durations, easings, the spring presets, the scene transition and the
 *  magic move, colour blend and motion blur defaults. */
export function MotionTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  const set = (path: string[], value: unknown) => onChange(setToken(tokens, ["motion", ...path], value));
  const springs = sectionKeys(tokens, "motion.spring");
  const [spring, setSpring] = useState<string | null>(null);
  const shownSpring = spring && springs.includes(spring) ? spring : (springs[0] ?? null);
  const kind = tokenAt(tokens, ["motion", "transition", "kind"]);
  const str = (path: string[]) => {
    const v = tokenAt(tokens, ["motion", ...path]);
    return typeof v === "string" ? v : undefined;
  };

  return (
    <>
      <Section title="Durations">
        {sectionKeys(tokens, "motion.duration").map((key) => (
          <Row key={key} label={tokenLabel(key)}>
            <NumberField
              label={`${tokenLabel(key)} duration`}
              value={tokenAt(tokens, ["motion", "duration", key]) as number}
              step={0.05}
              unit="s"
              onChange={(n) => set(["duration", key], n)}
            />
          </Row>
        ))}
      </Section>
      <Section title="Easings" note="Cubic-bezier points, as in CSS.">
        {sectionKeys(tokens, "motion.easing").map((key) => {
          const points = tokenAt(tokens, ["motion", "easing", key]);
          const at = Array.isArray(points) ? points : [];
          return (
            <div key={key} className="space-y-1">
              <div className="text-[12.5px] text-foreground">{tokenLabel(key)}</div>
              <div className="flex gap-1">
                {BEZIER.map((p, i) => (
                  <NumberField
                    key={p}
                    label={`${tokenLabel(key)} ${p}`}
                    value={typeof at[i] === "number" ? (at[i] as number) : undefined}
                    min={-10}
                    step={0.05}
                    className="[&_input]:w-[54px]"
                    onChange={(n) => set(["easing", key], BEZIER.map((_, j) => (j === i ? n : (at[j] ?? 0))))}
                  />
                ))}
              </div>
            </div>
          );
        })}
      </Section>
      {shownSpring && (
        <Section title="Springs" note="Components name a preset (gentle, snappy, bouncy) for things that pop in.">
          <Segmented label="Spring preset" options={options(springs)} value={shownSpring} onChange={setSpring} />
          {SPRING_PARTS.map((part) => (
            <Row key={part} label={tokenLabel(part)}>
              <NumberField
                label={`${shownSpring} ${part}`}
                value={tokenAt(tokens, ["motion", "spring", shownSpring, part]) as number}
                min={part === "damping" ? 0 : 0.01}
                step={part === "mass" ? 0.1 : 1}
                onChange={(n) => set(["spring", shownSpring, part], n)}
              />
            </Row>
          ))}
        </Section>
      )}
      <Section title="Scene transition" note="How a scene comes in, unless the video or the scene says otherwise.">
        <Row label="Kind">
          <NameSelect
            label="Transition kind"
            value={typeof kind === "string" ? kind : undefined}
            options={TRANSITION_KINDS}
            onChange={(k) => set(["transition", "kind"], k)}
          />
        </Row>
        <Row label="Length">
          <NumberField
            label="Transition length"
            value={tokenAt(tokens, ["motion", "transition", "dur"]) as number | undefined}
            step={0.05}
            unit="s"
            onChange={(n) => set(["transition", "dur"], n)}
          />
        </Row>
        {(kind === "push" || kind === "wipe") && (
          <Row label="Direction">
            <NameSelect
              label="Transition direction"
              value={str(["transition", "direction"])}
              options={DIRECTIONS}
              onChange={(d) => set(["transition", "direction"], d)}
            />
          </Row>
        )}
      </Section>
      <Section title="Morphs and blur" note="Unset ones fall back to Studio's own defaults.">
        <div className="space-y-1">
          <div className="text-[12.5px] text-foreground">Magic move</div>
          <Segmented label="Magic move" options={options(MAGIC_MOVE)} value={str(["magicMove"]) ?? ""} onChange={(v) => set(["magicMove"], v)} />
        </div>
        <div className="space-y-1">
          <div className="text-[12.5px] text-foreground">Colour blend</div>
          <Segmented label="Colour blend" options={MORPH_COLOR.map((v) => ({ value: v, label: v.toUpperCase() }))} value={str(["morphColor"]) ?? ""} onChange={(v) => set(["morphColor"], v)} />
        </div>
        <div className="space-y-1">
          <div className="text-[12.5px] text-foreground">Motion blur</div>
          <Segmented label="Motion blur" options={BLUR_OPTIONS} value={str(["motionBlur"]) ?? ""} onChange={(v) => set(["motionBlur"], v)} />
        </div>
      </Section>
    </>
  );
}
