import { ListControl } from "@/components/studio/components/inspector/ListControl";
import { FIELD, NumberInput, SELECT_FIELD } from "@/components/studio/panel/fields";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type { PropSpec } from "@/lib/studio/components";
import { cssColor, optionText, tokenSection } from "@/lib/studio/props";
import { cn } from "@/lib/utils";

export interface ControlProps {
  spec: PropSpec;
  /** The prop's name, or the list's, for the field's accessible name. */
  label: string;
  /** `undefined` while the prop is not set. */
  value: unknown;
  onChange: (value: unknown) => void;
  /** The chosen theme's tokens, for a `token` prop's names. */
  tokens: Record<string, unknown> | null;
}

/** The control for one prop's kind; the list control nests this one. */
export function PropControl(p: ControlProps) {
  const { spec } = p;
  switch (spec.kind) {
    case "str":
    case "latex":
      return <TextControl {...p} />;
    case "num":
      return <NumberControl {...p} />;
    case "bool":
      return <BoolControl {...p} />;
    case "oneOf":
      return <ChoiceControl {...p} spec={spec} />;
    case "token":
      return <TokenControl {...p} spec={spec} />;
    case "list":
      return <ListControl {...p} spec={spec} />;
  }
}

function TextControl({ spec, label, value, onChange }: ControlProps) {
  const text = typeof value === "string" ? value : "";
  const placeholder = typeof spec.default === "string" ? spec.default : undefined;
  if (text.includes("\n")) {
    return (
      <Textarea
        aria-label={label}
        value={text}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className={cn(FIELD, "h-auto min-h-0 py-1.5 leading-snug")}
      />
    );
  }
  return (
    <Input
      aria-label={label}
      value={text}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      className={FIELD}
    />
  );
}

function NumberControl({ spec, label, value, onChange }: ControlProps) {
  return (
    <NumberInput
      label={label}
      value={typeof value === "number" ? value : undefined}
      placeholder={typeof spec.default === "number" ? String(spec.default) : undefined}
      onChange={onChange}
      onClear={() => onChange(undefined)}
    />
  );
}

function BoolControl({ spec, label, value, onChange }: ControlProps) {
  const on = typeof value === "boolean" ? value : spec.default === true;
  return <Switch aria-label={label} checked={on} onCheckedChange={onChange} />;
}

/** An option is chosen by its index, since a value may be a number or an
 *  empty string, which Radix will not take. */
function ChoiceControl({ spec, label, value, onChange }: ControlProps & { spec: Extract<PropSpec, { kind: "oneOf" }> }) {
  const at = spec.values.findIndex((v) => Object.is(v, value) || JSON.stringify(v) === JSON.stringify(value));
  return (
    <Select value={at === -1 ? "" : String(at)} onValueChange={(i) => onChange(spec.values[Number(i)])}>
      <SelectTrigger size="sm" aria-label={label} className={SELECT_FIELD}>
        <SelectValue placeholder={spec.default !== undefined ? optionText(spec.default) : "Choose"} />
      </SelectTrigger>
      <SelectContent className="max-h-56">
        {spec.values.map((v, i) => (
          <SelectItem key={i} value={String(i)} className="text-[12.5px]">
            {optionText(v)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/** The names in the theme's section; a theme without the section (or one
 *  that does not parse) leaves a plain field. */
function TokenControl({ spec, label, value, onChange, tokens }: ControlProps & { spec: Extract<PropSpec, { kind: "token" }> }) {
  const section = tokenSection(tokens, spec.token);
  const names = section ? Object.keys(section) : [];
  const placeholder = typeof spec.default === "string" ? spec.default : undefined;
  if (!section || names.length === 0) {
    return (
      <Input
        aria-label={label}
        value={typeof value === "string" ? value : ""}
        placeholder={placeholder ?? spec.token}
        onChange={(e) => onChange(e.target.value)}
        className={FIELD}
      />
    );
  }
  const current = typeof value === "string" ? value : "";
  return (
    <Select value={names.includes(current) ? current : ""} onValueChange={onChange}>
      <SelectTrigger size="sm" aria-label={label} className={SELECT_FIELD}>
        <SelectValue placeholder={placeholder ?? `Choose from ${spec.token}`}>
          {current && <TokenName name={current} color={cssColor(section[current])} />}
        </SelectValue>
      </SelectTrigger>
      <SelectContent className="max-h-56">
        {names.map((n) => (
          <SelectItem key={n} value={n} className="text-[12.5px]">
            <TokenName name={n} color={cssColor(section[n])} />
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function TokenName({ name, color }: { name: string; color: string | null }) {
  return (
    <span className="flex items-center gap-2">
      {color && <span aria-hidden className="size-3 shrink-0 rounded-full ring-1 ring-border" style={{ backgroundColor: color }} />}
      {name}
    </span>
  );
}
