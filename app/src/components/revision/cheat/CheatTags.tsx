import { useMemo, useRef } from "react";
import { Plus } from "@phosphor-icons/react";
import { TagChip } from "@/components/projects/tags/TagChip";
import { TagPicker } from "@/components/projects/tags/TagPicker";
import { useCheatTags } from "@/hooks/data/useCheatTags";
import type { CheatTag } from "@/lib/revision";

/**
 * An entry's tags as the task tag chips. With `onChange` each chip takes an ×
 * and a + opens the task tag picker over the cheat sheet registry. Entries
 * hold names, so a name the registry lacks draws slate and a created one is
 * only a name until the entry is saved, when a trigger registers it.
 */
export function CheatTags({
  value,
  onChange,
  disabled,
}: {
  value: string[];
  onChange?: (names: string[]) => void;
  disabled?: boolean;
}) {
  const registry = useCheatTags();
  const created = useRef(new Map<number, CheatTag>());
  const nextId = useRef(-1);
  const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

  // Every name the entry carries, as a tag: the registry's, else a slate stand-in.
  const chosen = useMemo(
    () =>
      value.map((name, i): CheatTag =>
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
            const tag: CheatTag = { id: nextId.current--, name, color: "slate", position: Infinity };
            created.current.set(tag.id, tag);
            return tag;
          }}
        >
          <button
            type="button"
            aria-label="Add tag"
            className="inline-flex h-5 shrink-0 cursor-pointer items-center gap-1 rounded-full border border-dashed border-border px-1.5 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground data-[state=open]:bg-accent"
          >
            <Plus size={10} weight="bold" />
            {value.length === 0 && "Tag"}
          </button>
        </TagPicker>
      )}
    </>
  );
}
