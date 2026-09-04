import { useEffect, useState } from "react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  GROUP_LABELS,
  GROUP_ORDER,
  NODE_SPECS,
  type NodeSpec,
} from "@/components/automations/catalog";

/** What a spec is searchable by: its palette copy plus the extra terms in
 *  `keywords`, which exist so "cron", "deadline" or "switch" find a node whose
 *  copy never says the word. */
const haystack = (s: NodeSpec) =>
  [s.title, s.blurb, ...(s.keywords ?? [])].join(" ").toLowerCase();

/**
 * Every typed word has to appear somewhere in that text, in any order.
 *
 * Plain substrings, not cmdk's fuzzy score: a scored search reorders the
 * groups by how well they matched, so the shape of the palette changed with
 * every keystroke, and subsequence matching invented hits — "deadline" pulled
 * up "Read my files". With a catalog this size, narrowing beats ranking.
 */
function matches(s: NodeSpec, query: string): boolean {
  const text = haystack(s);
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((term) => text.includes(term));
}

/**
 * The searchable node palette.
 *
 * A list was fine at eight kinds and stops being fine as the catalog grows, so
 * this is a command palette: type to narrow, arrows to move, enter to insert.
 * `cmdk` runs the keyboard and the selection; the matching is ours.
 *
 * It hangs off a point in the canvas rather than the middle of the screen: it
 * is opened either by the toolbar button or by dropping a wire on empty
 * canvas, and both of those have a place they belong next to.
 */
export default function NodePalette({
  open,
  onOpenChange,
  at,
  accepts,
  onPick,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Where to hang the popup, in pixels relative to the canvas pane. */
  at: { x: number; y: number };
  /** Narrows the catalog. The wire-drag path passes the compatibility test for
   *  the port being dragged, so the palette only offers nodes that wire up. */
  accepts?: (spec: NodeSpec) => boolean;
  onPick: (spec: NodeSpec) => void;
}) {
  const [search, setSearch] = useState("");

  // A palette reopened on a different port should not still be showing the
  // last query's results.
  useEffect(() => {
    if (open) setSearch("");
  }, [open]);

  const specs = NODE_SPECS.filter((s) => (!accepts || accepts(s)) && matches(s, search));

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverAnchor asChild>
        <div className="pointer-events-none absolute size-0" style={{ left: at.x, top: at.y }} />
      </PopoverAnchor>
      {/* Radix parks focus on the content's first tabbable child on open, and
          that is the search field — so the palette is typeable the instant it
          appears, and Escape hands focus back to the canvas. */}
      <PopoverContent align="start" sideOffset={0} className="w-[288px] overflow-hidden p-0">
        {/* The list is already narrowed above, so cmdk is left to do the
            keyboard and the highlight only. */}
        <Command loop shouldFilter={false}>
          <CommandInput
            autoFocus
            value={search}
            onValueChange={setSearch}
            placeholder="Search nodes…"
          />
          <CommandList>
            <CommandEmpty>No node matches.</CommandEmpty>
            {GROUP_ORDER.map((group) => {
              const inGroup = specs.filter((s) => s.group === group);
              if (inGroup.length === 0) return null;
              return (
                <CommandGroup key={group} heading={GROUP_LABELS[group]}>
                  {inGroup.map((spec) => (
                    <CommandItem
                      key={spec.kind}
                      value={spec.kind}
                      onSelect={() => onPick(spec)}
                      className="items-start gap-2"
                    >
                      <spec.icon size={13} className="mt-0.5 shrink-0 text-muted-foreground" />
                      <span className="min-w-0">
                        <span className="block text-xs text-foreground">{spec.title}</span>
                        <span className="block text-[11px] leading-snug text-muted-foreground">
                          {spec.blurb}
                        </span>
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              );
            })}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
