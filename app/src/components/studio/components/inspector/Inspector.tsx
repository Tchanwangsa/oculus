import { ArrowCounterClockwise, X } from "@phosphor-icons/react";

import { PropControl } from "@/components/studio/components/inspector/PropControl";
import { Button } from "@/components/ui/button";
import type { NamedProp, PropSpec } from "@/lib/studio/components";
import { unsettable, type PropValues } from "@/lib/studio/props";

const KIND_NAME: Record<PropSpec["kind"], string> = {
  str: "text",
  num: "number",
  bool: "on or off",
  list: "list",
  oneOf: "choice",
  token: "theme name",
  latex: "LaTeX",
};

/** A required prop's value is "" only when it is text; any other empty
 *  edit means it is not set. */
function nextValues(values: PropValues, prop: NamedProp, value: unknown): PropValues {
  const next = { ...values };
  const text = prop.kind === "str" || prop.kind === "latex";
  const empty = value === undefined || (text && value === "" && prop.optional);
  if (empty) delete next[prop.name];
  else next[prop.name] = value;
  return next;
}

/**
 * The props form generated from the component's `props` schema. Each edit
 * hands back the whole set; a prop that is not in it is not set, so it takes
 * its default (shown as the placeholder). `onReset` goes back to the chosen
 * example's props.
 */
export function Inspector({
  props,
  values,
  tokens,
  onChange,
  onReset,
  canReset,
}: {
  props: NamedProp[];
  values: PropValues;
  tokens: Record<string, unknown> | null;
  onChange: (values: PropValues) => void;
  onReset: () => void;
  canReset: boolean;
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-[11px] font-medium tracking-wide text-muted-foreground">Props</h3>
        <Button type="button" variant="ghost" size="xs" disabled={!canReset} onClick={onReset}>
          <ArrowCounterClockwise size={12} />
          Reset to example
        </Button>
      </div>
      {props.length === 0 && <p className="text-[12.5px] text-muted-foreground">This component takes no props.</p>}
      {props.map((prop) => {
        const set = prop.name in values;
        const hint = set ? KIND_NAME[prop.kind] : prop.default !== undefined ? "default" : prop.optional ? "not set" : "required";
        return (
          <div key={prop.name} className="space-y-1.5">
            <div className="flex items-center gap-1.5">
              <span className="min-w-0 truncate text-[12.5px] font-medium text-foreground">{prop.name}</span>
              <span className="text-[11px] text-muted-foreground">{hint}</span>
              <div className="flex-1" />
              {set && unsettable(prop) && (
                <button
                  type="button"
                  aria-label={`Unset ${prop.name}`}
                  title="Unset"
                  onClick={() => onChange(nextValues(values, prop, undefined))}
                  className="flex size-5 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                >
                  <X size={11} />
                </button>
              )}
            </div>
            <PropControl
              spec={prop}
              value={values[prop.name]}
              tokens={tokens}
              onChange={(v) => onChange(nextValues(values, prop, v))}
            />
          </div>
        );
      })}
    </div>
  );
}
