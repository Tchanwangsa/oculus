import { Segmented } from "@/components/studio/style/Segmented";
import { Note, tokenLabel } from "@/components/studio/themes/inspector/Fields";
import { InlineField, LabeledField, NameSelect, NumberInput, SubLabel } from "@/components/studio/panel/fields";
import {
  DIRECTIONS,
  MAGIC_MOVE,
  MORPH_COLOR,
  MOTION_BLUR,
  TRANSITION_KINDS,
  setToken,
  tokenAt,
  type Tokens,
} from "@/lib/studio/themeDraft";

const BLUR_LABEL: Record<(typeof MOTION_BLUR)[number], string> = { finals: "Finals", off: "Off", always: "Always" };
const BLUR_OPTIONS = MOTION_BLUR.map((v) => ({ value: v as string, label: BLUR_LABEL[v] }));

const COLOR_BLEND = MORPH_COLOR.map((v) => ({ value: v as string, label: v.toUpperCase() }));

const options = (names: readonly string[]) => names.map((n) => ({ value: n, label: tokenLabel(n) }));

/** The scene transition's kind, length and direction, then the magic move,
 *  colour blend and motion blur defaults. */
export function TransitionTokens({ tokens, onChange }: { tokens: Tokens; onChange: (next: Tokens) => void }) {
  const set = (path: string[], value: unknown) => onChange(setToken(tokens, ["motion", ...path], value));
  const kind = tokenAt(tokens, ["motion", "transition", "kind"]);
  const str = (path: string[]) => {
    const v = tokenAt(tokens, ["motion", ...path]);
    return typeof v === "string" ? v : undefined;
  };

  return (
    <>
      <div className="space-y-1">
        <InlineField label="Kind">
          <NameSelect
            label="Transition kind"
            value={typeof kind === "string" ? kind : undefined}
            options={TRANSITION_KINDS}
            onChange={(k) => set(["transition", "kind"], k)}
          />
        </InlineField>
        <InlineField label="Length">
          <NumberInput
            label="Transition length"
            value={tokenAt(tokens, ["motion", "transition", "dur"]) as number | undefined}
            min={0}
            step={0.05}
            unit="s"
            onChange={(n) => set(["transition", "dur"], n)}
          />
        </InlineField>
        {(kind === "push" || kind === "wipe") && (
          <InlineField label="Direction">
            <NameSelect
              label="Transition direction"
              value={str(["transition", "direction"])}
              options={DIRECTIONS}
              onChange={(d) => set(["transition", "direction"], d)}
            />
          </InlineField>
        )}
      </div>
      <Note>How a scene comes in, unless the video or the scene says otherwise.</Note>
      <SubLabel className="pt-3">Morphs and blur</SubLabel>
      <div className="space-y-2.5">
        <Choice label="Magic move" options={options(MAGIC_MOVE)} value={str(["magicMove"])} onChange={(v) => set(["magicMove"], v)} />
        <Choice label="Colour blend" options={COLOR_BLEND} value={str(["morphColor"])} onChange={(v) => set(["morphColor"], v)} />
        <Choice label="Motion blur" options={BLUR_OPTIONS} value={str(["motionBlur"])} onChange={(v) => set(["motionBlur"], v)} />
      </div>
      <Note>Motion blur on finals means on final renders only. An unset choice falls back to Studio's own default.</Note>
    </>
  );
}

/** A labelled row of pills; an unset token lights none. */
function Choice({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: ReadonlyArray<{ value: string; label: string }>;
  value: string | undefined;
  onChange: (value: string) => void;
}) {
  return (
    <LabeledField label={label}>
      <Segmented label={label} options={options} value={value ?? ""} onChange={onChange} />
    </LabeledField>
  );
}
