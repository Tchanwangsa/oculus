import { CircleNotch } from "@phosphor-icons/react";

import type { OpencodeProvider } from "@/lib/harness/opencodeAuth";
import { providerOf, type OpencodeCatalogue } from "@/lib/harness/opencodeCatalogue";
import type { HarnessModel } from "@/lib/harness";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { FilterChecklist } from "@/components/ui/filter/FilterChecklist";
import { FilterChips } from "@/components/ui/filter/FilterChips";
import { FilterPopover, FilterRow } from "@/components/ui/filter/FilterPopover";
import { GridTable } from "@/components/ui/table/GridTable";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { isHidden } from "./useOpencode";
import { SearchField, SortHeader, TableEmpty } from "./parts";
import { COLS, type ModelsFocus, type Scope } from "./models/constants";
import { ModelRow } from "./models/ModelRow";
import { useModelsView } from "./models/useModelsView";

export type { ModelsFocus } from "./models/constants";

const SCOPES: ReadonlyArray<{ value: Scope; label: string }> = [
  { value: "all", label: "Any status" },
  { value: "offered", label: "Offered" },
  { value: "hidden", label: "Hidden" },
  { value: "unavailable", label: "Unavailable" },
];

/**
 * Every model opencode lists for the signed-in, unhidden providers, one row
 * each, ticked when Chat's picker offers it. A blocked row greys out with a
 * disabled tick and says why on hover; a hidden model is ticked off. Filters
 * are provider (who serves it), maker (who built it) and status, applied ones
 * listed as chips. Facts are opencode's catalogue, a free read: null is
 * unknown and draws "—".
 */
export function ModelsTable({
  models,
  providers,
  catalogue,
  focus,
  error,
  onHidden,
  onShowProviders,
}: {
  /** Every model opencode lists, or null before the read lands. */
  models: HarnessModel[] | null;
  /** For display names; null until opencode answers, when ids stand in. */
  providers: OpencodeProvider[] | null;
  catalogue: OpencodeCatalogue | null;
  focus: ModelsFocus;
  error: string | null;
  onHidden: (ids: string[], hidden: boolean) => void;
  onShowProviders: () => void;
}) {
  const v = useModelsView(models, providers, catalogue, focus);
  const { query, setQuery, providerSet, makerSet, scope, sorted, showable, hideable } = v;

  const filtering = providerSet.size > 0 || makerSet.size > 0 || scope !== "all";
  const q = query.trim();
  const empty =
    models === null ? (
      <TableEmpty
        message={
          <span className="inline-flex items-center gap-2">
            <CircleNotch size={12} className="animate-spin" />
            Reading opencode's catalogue…
          </span>
        }
      />
    ) : v.rows.length === 0 ? (
      <TableEmpty message="No provider is signed in yet, so there is no model to offer.">
        <Button variant="outline" size="sm" onClick={onShowProviders}>
          Add a provider
        </Button>
      </TableEmpty>
    ) : sorted.length > 0 ? null : q ? (
      <TableEmpty message={`No model matches “${q}”${filtering ? " under these filters" : ""}.`}>
        <Button variant="outline" size="sm" onClick={() => setQuery("")}>
          Clear search
        </Button>
      </TableEmpty>
    ) : (
      <TableEmpty message="No model matches these filters.">
        <Button variant="outline" size="sm" onClick={v.clearFilters}>
          Clear filters
        </Button>
      </TableEmpty>
    );

  const chips = [
    ...[...providerSet].map((id) => ({
      key: `p:${id}`,
      label: v.providerName(id),
      onRemove: () => v.setProviderSet(without(providerSet, id)),
    })),
    ...[...makerSet].map((slug) => ({
      key: `m:${slug}`,
      label: v.makerLabel(slug),
      onRemove: () => v.setMakerSet(without(makerSet, slug)),
    })),
    ...(scope === "all"
      ? []
      : [{ key: "s", label: scopeLabel(scope), onRemove: () => v.onScope("all") }]),
  ];

  // Ticked when every listed model is offered; acts on all of them, every page.
  const allState =
    showable.length === 0 && hideable.length > 0
      ? true
      : showable.length > 0 && hideable.length > 0
        ? "indeterminate"
        : false;
  const loaded = models !== null && v.rows.length > 0;

  return (
    <>
      {/* Fixed height so switching tabs doesn't jump the table. */}
      <div className="flex h-12 shrink-0 items-center gap-2 px-5">
        <SearchField
          value={query}
          onChange={setQuery}
          placeholder={`Search ${v.rows.length} models`}
          disabled={models === null}
        />
        <FilterPopover
          label={countLabel(providerSet, v.providerName, "All providers", "providers")}
          active={providerSet.size > 0}
          className="w-64"
        >
          {() => (
            <FilterChecklist
              options={v.providerOptions}
              selected={providerSet}
              onChange={v.setProviderSet}
              noun="providers"
            />
          )}
        </FilterPopover>
        <FilterPopover
          label={countLabel(makerSet, v.makerLabel, "All makers", "makers")}
          active={makerSet.size > 0}
          className="w-64"
        >
          {() => (
            <FilterChecklist
              options={v.makerOptions}
              selected={makerSet}
              onChange={v.setMakerSet}
              noun="makers"
            />
          )}
        </FilterPopover>
        <FilterPopover label={scopeLabel(scope)} active={scope !== "all"} className="w-40">
          {(close) =>
            SCOPES.map((s) => (
              <FilterRow
                key={s.value}
                label={s.label}
                selected={scope === s.value}
                onPick={() => {
                  close();
                  v.onScope(s.value);
                }}
              />
            ))
          }
        </FilterPopover>
      </div>

      <FilterChips chips={chips} onClear={v.clearFilters} />

      {error && (
        <p data-selectable className="shrink-0 px-5 pb-2.5 text-xs text-destructive">
          {error}
        </p>
      )}

      <div className="min-h-0 flex-1">
        <GridTable
          cols={COLS}
          header={
            <>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Checkbox
                    aria-label={allState === true ? "Hide every listed model" : "Offer every listed model"}
                    checked={allState}
                    disabled={!loaded || (showable.length === 0 && hideable.length === 0)}
                    onCheckedChange={(next) =>
                      next === true ? onHidden(showable, false) : onHidden(hideable, true)
                    }
                  />
                </TooltipTrigger>
                <TooltipContent>
                  {allState === true ? "Hide every listed model" : "Offer every listed model"}
                </TooltipContent>
              </Tooltip>
              <SortHeader label="Model" column="id" sort={v.sort} onSort={v.onSort} />
              <SortHeader label="Provider" column="provider" sort={v.sort} onSort={v.onSort} />
              <SortHeader label="Input $/M" column="input" sort={v.sort} onSort={v.onSort} end />
              <SortHeader label="Output $/M" column="output" sort={v.sort} onSort={v.onSort} end />
              <SortHeader label="Context" column="context" sort={v.sort} onSort={v.onSort} end />
              <SortHeader label="Max output" column="maxOutput" sort={v.sort} onSort={v.onSort} end />
              <span className="text-[11px] font-medium text-muted-foreground">Capabilities</span>
              <SortHeader label="Released" column="released" sort={v.sort} onSort={v.onSort} end />
            </>
          }
          pagination={{
            page: v.page,
            pageCount: v.pageCount,
            onPage: v.setPage,
            total: sorted.length,
            unit: "model",
            summary: loaded ? v.status : undefined,
          }}
        >
          {empty ?? (
            <div className="divide-y divide-border-subtle">
              {v.pageRows.map((m) => (
                <ModelRow
                  key={m.id}
                  model={m}
                  provider={v.providerName(providerOf(m.id))}
                  hidden={isHidden(catalogue, m.id)}
                  onHidden={(hide) => onHidden([m.id], hide)}
                />
              ))}
            </div>
          )}
        </GridTable>
      </div>
    </>
  );
}

function scopeLabel(scope: Scope): string {
  return SCOPES.find((s) => s.value === scope)!.label;
}

/** "All providers", the one picked, or "3 providers". */
function countLabel(
  set: ReadonlySet<string>,
  name: (value: string) => string,
  none: string,
  plural: string,
): string {
  if (set.size === 0) return none;
  if (set.size === 1) return name([...set][0]);
  return `${set.size} ${plural}`;
}

function without(set: ReadonlySet<string>, value: string): Set<string> {
  const next = new Set(set);
  next.delete(value);
  return next;
}
