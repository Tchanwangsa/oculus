import { CircleNotch } from "@phosphor-icons/react";

import type { OpencodeProvider } from "@/lib/harness/opencodeAuth";
import { providerOf, type OpencodeCatalogue } from "@/lib/harness/opencodeCatalogue";
import type { HarnessModel } from "@/lib/harness";
import { Button } from "@/components/ui/button";
import { GridTable } from "@/components/ui/table/GridTable";
import { PillTabs } from "@/components/ui/table/PillTabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { isHidden } from "./useOpencode";
import { SearchField, SortHeader, TableEmpty } from "./parts";
import { ALL, COLS, type ModelsFocus } from "./models/constants";
import { ModelRow } from "./models/ModelRow";
import { useModelsView } from "./models/useModelsView";

export type { ModelsFocus } from "./models/constants";

/**
 * Every model opencode lists for the signed-in, unhidden providers, one row
 * each, ticked when Chat's picker offers it. A blocked row keeps its reason and
 * a disabled tick and never reads as available; a hidden model is ticked off.
 * Facts are opencode's catalogue, a free read: null is unknown and draws "—".
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
  const {
    query,
    setQuery,
    provider,
    setProvider,
    sort,
    picked,
    scope,
    providerName,
    rows,
    providerIds,
    onScope,
    sorted,
    page,
    pageCount,
    setPage,
    pageRows,
    onSort,
    status,
    showable,
    hideable,
  } = useModelsView(models, providers, catalogue, focus);

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
    ) : rows.length === 0 ? (
      <TableEmpty message="No provider is signed in yet, so there is no model to offer.">
        <Button variant="outline" size="sm" onClick={onShowProviders}>
          Add a provider
        </Button>
      </TableEmpty>
    ) : sorted.length > 0 ? null : q ? (
      <TableEmpty
        message={
          picked
            ? `No ${picked.scope} model matches “${q}”.`
            : `No model matches “${q}”.`
        }
      >
        <Button variant="outline" size="sm" onClick={() => setQuery("")}>
          Clear search
        </Button>
      </TableEmpty>
    ) : picked ? (
      <TableEmpty message={`No model here is ${picked.scope}.`}>
        <Button variant="outline" size="sm" onClick={() => onScope("all")}>
          Show every model
        </Button>
      </TableEmpty>
    ) : (
      <TableEmpty message={`opencode lists no models for ${providerName(provider)}.`}>
        <Button variant="outline" size="sm" onClick={() => setProvider(ALL)}>
          Show every provider
        </Button>
      </TableEmpty>
    );

  return (
    <>
      {/* Fixed height so switching tabs doesn't jump the table. */}
      <div className="flex h-12 shrink-0 items-center gap-3 px-5">
        <SearchField
          value={query}
          onChange={setQuery}
          placeholder={`Search ${rows.length} models`}
          disabled={models === null}
        />
        <Select value={provider} onValueChange={setProvider}>
          <SelectTrigger aria-label="Provider" size="sm" className="h-7 w-40 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>All providers</SelectItem>
            {providerIds.map((id) => (
              <SelectItem key={id} value={id}>
                {providerName(id)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <PillTabs
          tabs={[
            { value: "all", label: "All" },
            { value: "offered", label: "Offered" },
            { value: "hidden", label: "Hidden" },
          ]}
          value={scope}
          onChange={onScope}
        />
        <span className="flex-1" />
        {models !== null && rows.length > 0 && (
          <>
            <span className="min-w-0 truncate text-[11px] text-muted-foreground tabular-nums">
              {status}
            </span>
            <div className="flex shrink-0 items-center gap-1">
              <Button
                variant="ghost"
                size="xs"
                disabled={showable.length === 0}
                onClick={() => onHidden(showable, false)}
              >
                Show all
              </Button>
              <Button
                variant="ghost"
                size="xs"
                disabled={hideable.length === 0}
                onClick={() => onHidden(hideable, true)}
              >
                Hide all
              </Button>
            </div>
          </>
        )}
      </div>

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
              <span />
              <SortHeader label="Model" column="id" sort={sort} onSort={onSort} />
              <SortHeader label="Provider" column="provider" sort={sort} onSort={onSort} />
              <SortHeader label="Input $/M" column="input" sort={sort} onSort={onSort} end />
              <SortHeader label="Output $/M" column="output" sort={sort} onSort={onSort} end />
              <SortHeader label="Context" column="context" sort={sort} onSort={onSort} end />
              <SortHeader label="Max output" column="maxOutput" sort={sort} onSort={onSort} end />
              <span className="text-[11px] font-medium text-muted-foreground">Capabilities</span>
              <SortHeader label="Released" column="released" sort={sort} onSort={onSort} end />
            </>
          }
          pagination={{ page, pageCount, onPage: setPage, total: sorted.length, unit: "model" }}
        >
          {empty ?? (
            <div className="divide-y divide-border-subtle">
              {pageRows.map((m) => (
                <ModelRow
                  key={m.id}
                  model={m}
                  provider={providerName(providerOf(m.id))}
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
