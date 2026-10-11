import { useEffect, useState } from "react";
import { Plus, X } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type { PropSpec } from "@/lib/studio/components";
import { blankValue, cssColor, optionText, tokenSection } from "@/lib/studio/props";

export interface ControlProps {
  spec: PropSpec;
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

function TextControl({ spec, value, onChange }: ControlProps) {
  const text = typeof value === "string" ? value : "";
  const placeholder = typeof spec.default === "string" ? spec.default : undefined;
  if (text.includes("\n")) {
    return (
      <Textarea
        aria-label="Text"
        value={text}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="min-h-0"
      />
    );
  }
  return (
    <Input aria-label="Text" value={text} placeholder={placeholder} onChange={(e) => onChange(e.target.value)} />
  );
}

/** A draft string, so "1." and "-" can be typed on the way to a number. */
function NumberControl({ spec, value, onChange }: ControlProps) {
  const [draft, setDraft] = useState(typeof value === "number" ? String(value) : "");
  useEffect(() => {
    // An outside change (a reset, another example) replaces the draft.
    setDraft((d) => (Number(d) === value && d !== "" ? d : typeof value === "number" ? String(value) : ""));
  }, [value]);
  return (
    <Input
      aria-label="Number"
      type="number"
      inputMode="decimal"
      value={draft}
      placeholder={typeof spec.default === "number" ? String(spec.default) : undefined}
      onChange={(e) => {
        setDraft(e.target.value);
        const n = e.target.value.trim() === "" ? NaN : Number(e.target.value);
        onChange(Number.isFinite(n) ? n : undefined);
      }}
    />
  );
}

function BoolControl({ spec, value, onChange }: ControlProps) {
  const on = typeof value === "boolean" ? value : spec.default === true;
  return <Switch aria-label="On or off" checked={on} onCheckedChange={onChange} />;
}

/** An option is chosen by its index, since a value may be a number or an
 *  empty string, which Radix will not take. */
function ChoiceControl({ spec, value, onChange }: ControlProps & { spec: Extract<PropSpec, { kind: "oneOf" }> }) {
  const at = spec.values.findIndex((v) => Object.is(v, value) || JSON.stringify(v) === JSON.stringify(value));
  return (
    <Select
      value={at === -1 ? "" : String(at)}
      onValueChange={(i) => onChange(spec.values[Number(i)])}
    >
      <SelectTrigger size="sm" aria-label="Choice" className="w-full text-[13px]">
        <SelectValue placeholder={spec.default !== undefined ? optionText(spec.default) : "Choose"} />
      </SelectTrigger>
      <SelectContent className="max-h-56">
        {spec.values.map((v, i) => (
          <SelectItem key={i} value={String(i)} className="text-[13px]">
            {optionText(v)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/** The names in the theme's section; a theme without the section (or one
 *  that does not parse) leaves a plain field. */
function TokenControl({ spec, value, onChange, tokens }: ControlProps & { spec: Extract<PropSpec, { kind: "token" }> }) {
  const section = tokenSection(tokens, spec.token);
  const names = section ? Object.keys(section) : [];
  if (!section || names.length === 0) {
    return (
      <Input
        aria-label={`A name in ${spec.token}`}
        value={typeof value === "string" ? value : ""}
        placeholder={typeof spec.default === "string" ? spec.default : spec.token}
        onChange={(e) => onChange(e.target.value)}
      />
    );
  }
  const current = typeof value === "string" ? value : "";
  return (
    <Select value={names.includes(current) ? current : ""} onValueChange={onChange}>
      <SelectTrigger size="sm" aria-label={`A name in ${spec.token}`} className="w-full text-[13px]">
        <SelectValue placeholder={typeof spec.default === "string" ? spec.default : `Choose from ${spec.token}`}>
          {current && <TokenName name={current} color={cssColor(section[current])} />}
        </SelectValue>
      </SelectTrigger>
      <SelectContent className="max-h-56">
        {names.map((n) => (
          <SelectItem key={n} value={n} className="text-[13px]">
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

function ListControl({ spec, value, onChange, tokens }: ControlProps & { spec: Extract<PropSpec, { kind: "list" }> }) {
  const items = Array.isArray(value) ? value : [];
  const set = (next: unknown[]) => onChange(next);
  return (
    <div className="space-y-1.5">
      {items.map((item, i) => (
        <div key={i} className="flex items-start gap-1.5">
          <div className="min-w-0 flex-1">
            <PropControl
              spec={spec.of}
              value={item}
              tokens={tokens}
              onChange={(v) => set(items.map((x, j) => (j === i ? v : x)))}
            />
          </div>
          <button
            type="button"
            aria-label={`Remove item ${i + 1}`}
            title="Remove"
            onClick={() => set(items.filter((_, j) => j !== i))}
            className="mt-1 flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X size={12} />
          </button>
        </div>
      ))}
      <Button type="button" variant="outline" size="xs" onClick={() => set([...items, blankValue(spec.of, tokens)])}>
        <Plus size={12} weight="bold" />
        Add
      </Button>
    </div>
  );
}
