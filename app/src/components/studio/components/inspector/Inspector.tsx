import { AddItem } from "@/components/studio/components/inspector/ListControl";
import { PropControl } from "@/components/studio/components/inspector/PropControl";
import { PropRow } from "@/components/studio/components/inspector/PropRow";
import { CollapsibleSection } from "@/components/ui/CollapsibleSection";
import { useInspectorSections } from "@/hooks/studio/useInspectorSections";
import type { NamedProp } from "@/lib/studio/components";
import { blankValue, changed, groupProps, unsettable, type PropValues } from "@/lib/studio/props";

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
 * The props form generated from the component's `props` schema, in sections
 * (`groupProps`) that fold and are remembered. Each edit hands back the whole
 * set; a prop that is not in it is not set, so it takes its default (shown as
 * the placeholder).
 */
export function Inspector({
  props,
  values,
  tokens,
  onChange,
}: {
  props: NamedProp[];
  values: PropValues;
  tokens: Record<string, unknown> | null;
  onChange: (values: PropValues) => void;
}) {
  const sections = useInspectorSections("oculus-studio-components-sections");
  if (props.length === 0) {
    return <p className="py-3 text-[12.5px] text-muted-foreground">This component takes no props.</p>;
  }
  return (
    <div>
      {groupProps(props).map((group) => (
        <CollapsibleSection
          key={group.id}
          title={group.title}
          count={group.props.length}
          marked={group.props.some((p) => changed(p, values))}
          open={sections.isOpen(group.id)}
          onOpenChange={(open) => sections.setOpen(group.id, open)}
        >
          {group.props.map((prop) => {
            const set = prop.name in values;
            const items = prop.kind === "list" && Array.isArray(values[prop.name]) ? (values[prop.name] as unknown[]) : null;
            // An empty list is its name and an Add, with no body under it.
            const empty = prop.kind === "list" && (items?.length ?? 0) === 0;
            return (
              <PropRow
                key={prop.name}
                prop={prop}
                set={set}
                changed={changed(prop, values)}
                count={items?.length}
                onUnset={set && unsettable(prop) ? () => onChange(nextValues(values, prop, undefined)) : null}
                action={
                  empty && prop.kind === "list" ? (
                    <AddItem label={prop.name} onClick={() => onChange(nextValues(values, prop, [blankValue(prop.of, tokens)]))} />
                  ) : undefined
                }
              >
                {!empty && (
                  <PropControl
                    spec={prop}
                    label={prop.name}
                    value={values[prop.name]}
                    tokens={tokens}
                    onChange={(v) => onChange(nextValues(values, prop, v))}
                  />
                )}
              </PropRow>
            );
          })}
        </CollapsibleSection>
      ))}
    </div>
  );
}
