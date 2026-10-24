import { useMemo, useRef } from "react";
import { Plus } from "@phosphor-icons/react";
import type { DbTaskTag } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import { PILL } from "../fields/pill";
import { TagChip } from "./TagChip";
import { TagPicker } from "./TagPicker";

/**
 * Tags held as names, drawn as tag chips over a registry. With `onChange`
 * each chip takes an × and a + opens the tag picker. A name the registry
 * lacks draws slate, and a created one is only a name until its row is
 * saved, when a database trigger registers it. `pill` sizes the chips and
 * the + to the property pills beside them.
 */
export function NameTags({
  registry,
  value,
  onChange,
  disabled,
  size = "chip",
}: {
  registry: DbTaskTag[];
  value: string[];
  onChange?: (names: string[]) => void;
  disabled?: boolean;
  size?: "chip" | "pill";
}) {
  const created = useRef(new Map<number, DbTaskTag>());
  const nextId = useRef(-1);
  const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

  // Every name the row carries, as a tag: the registry's, else a slate stand-in.
  const chosen = useMemo(
    () =>
      value.map((name, i): DbTaskTag =>
        registry.find((tag) => same(tag.name, name)) ??
        [...created.current.values()].find((tag) => same(tag.name, name)) ?? {
          id: -1000 - i,
          name,
          color: "slate",
          position: Infinity,
        },
      ),
    [value, registry],
  );
  const options = useMemo(
    () => [...registry, ...chosen.filter((tag) => !registry.some((r) => r.id === tag.id))],
    [registry, chosen],
  );

  const edit = onChange && !disabled;
  const toNames = (ids: number[]) =>
    ids.flatMap((id) => options.find((tag) => tag.id === id) ?? created.current.get(id) ?? []).map((tag) => tag.name);

  return (
    <>
      {chosen.map((tag) => (
        <TagChip
          key={tag.name}
          tag={tag}
          className={size === "pill" ? "h-7 px-2.5 text-xs" : undefined}
          onRemove={edit ? () => onChange(value.filter((name) => !same(name, tag.name))) : undefined}
        />
      ))}
      {edit && (
        <TagPicker
          tags={options}
          value={chosen.map((tag) => tag.id)}
          onChange={(ids) => onChange(toNames(ids))}
          onCreate={async (raw) => {
            const name = raw.trim().replace(/\s+/g, " ").replace(/^#/, "");
            const known = options.find((tag) => same(tag.name, name));
            if (known) return known;
            const tag: DbTaskTag = { id: nextId.current--, name, color: "slate", position: Infinity };
            created.current.set(tag.id, tag);
            return tag;
          }}
        >
          <button
            type="button"
            aria-label="Add tag"
            className={size === "pill"
              ? cn(PILL, "text-muted-foreground", value.length > 0 && "size-7 justify-center px-0")
              : "inline-flex h-5 shrink-0 cursor-pointer items-center gap-1 rounded-full border border-dashed border-border px-1.5 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground data-[state=open]:bg-accent"}
          >
            <Plus size={size === "pill" ? 12 : 10} weight="bold" />
            {value.length === 0 && "Tag"}
          </button>
        </TagPicker>
      )}
    </>
  );
}
