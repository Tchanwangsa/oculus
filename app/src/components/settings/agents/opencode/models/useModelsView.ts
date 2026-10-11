import { useCallback, useEffect, useMemo, useState } from "react";
import type { OpencodeProvider } from "@/lib/harness/opencodeAuth";
import { isZen, providerOf, type OpencodeCatalogue } from "@/lib/harness/opencodeCatalogue";
import type { HarnessModel } from "@/lib/harness";
import type { FilterOption } from "@/components/ui/filter/FilterChecklist";
import { usePagedRows } from "@/components/ui/table/TablePagination";
import { blockedBecause, isHidden } from "../useOpencode";
import type { SortDir } from "../parts";
import { PAGE_SIZE, UNKNOWN_MAKER, type ModelsFocus, type Scope, type SortKey } from "./constants";
import { makerName, makerOf } from "./maker";

type Facet = "provider" | "maker" | "status";

/** The table's filters, sort, pagination and the counts drawn from them. */
export function useModelsView(
  models: HarnessModel[] | null,
  providers: OpencodeProvider[] | null,
  catalogue: OpencodeCatalogue | null,
  focus: ModelsFocus,
) {
  const [query, setQuery] = useState("");
  const [providerSet, setProviderSet] = useState<Set<string>>(() => focusSet(focus));
  const [makerSet, setMakerSet] = useState<Set<string>>(() => new Set());
  const [sort, setSort] = useState<{ key: SortKey; dir: SortDir }>({ key: "id", dir: "asc" });
  // *Offered* and *Hidden* snapshot the ticks when chosen, not live, so
  // unticking a row doesn't pull it out from under the click.
  const [picked, setPicked] = useState<{ scope: Exclude<Scope, "all">; ids: Set<string> } | null>(
    null,
  );
  const scope: Scope = picked?.scope ?? "all";

  useEffect(() => {
    setQuery("");
    setPicked(null);
    setMakerSet(new Set());
    setProviderSet(new Set(focus.provider ? [focus.provider] : []));
  }, [focus.n, focus.provider]);

  const names = useMemo(
    () => new Map((providers ?? []).map((p) => [p.id, p.name])),
    [providers],
  );
  const providerName = useCallback((id: string) => names.get(id) ?? id, [names]);

  // A hidden provider's models never reach the picker, and Zen's never run, so neither is listed.
  const rows = useMemo(() => {
    if (!models) return [];
    const hiddenIds = new Set(catalogue?.hiddenProviders ?? []);
    return models.filter((m) => !isZen(m.id) && !hiddenIds.has(providerOf(m.id)));
  }, [models, catalogue]);

  const makers = useMemo(
    () => new Map(rows.map((m) => [m.id, makerOf(m) ?? UNKNOWN_MAKER])),
    [rows],
  );
  const makerLabel = useCallback(
    (slug: string) => (slug === UNKNOWN_MAKER ? "Unknown maker" : makerName(slug)),
    [],
  );

  // Every filter but `skip`, so a facet's own counts don't collapse to its pick.
  const matches = useCallback(
    (m: HarnessModel, skip?: Facet) => {
      const q = query.trim().toLowerCase();
      if (q && !m.label.toLowerCase().includes(q) && !m.id.toLowerCase().includes(q)) return false;
      if (skip !== "provider" && providerSet.size > 0 && !providerSet.has(providerOf(m.id))) {
        return false;
      }
      if (skip !== "maker" && makerSet.size > 0 && !makerSet.has(makers.get(m.id)!)) return false;
      if (skip !== "status" && picked && !picked.ids.has(m.id)) return false;
      return true;
    },
    [query, providerSet, makerSet, picked, makers],
  );

  const providerOptions = useMemo(
    () => facetOptions(rows, (m) => providerOf(m.id), providerName, (m) => matches(m, "provider")),
    [rows, providerName, matches],
  );
  const makerOptions = useMemo(
    () => facetOptions(rows, (m) => makers.get(m.id)!, makerLabel, (m) => matches(m, "maker")),
    [rows, makers, makerLabel, matches],
  );

  const inStatus = (m: HarnessModel, s: Exclude<Scope, "all">) => {
    const blocked = blockedBecause(m) !== null;
    if (s === "unavailable") return blocked;
    return !blocked && isHidden(catalogue, m.id) === (s === "hidden");
  };

  const onScope = (next: Scope) =>
    setPicked(
      next === "all"
        ? null
        : {
            scope: next,
            ids: new Set(rows.filter((m) => inStatus(m, next)).map((m) => m.id)),
          },
    );

  const filtered = useMemo(() => rows.filter((m) => matches(m)), [rows, matches]);

  const sorted = useMemo(() => {
    const value = (m: HarnessModel): string | number | null => {
      const f = m.facts;
      switch (sort.key) {
        case "id":
          return m.id;
        case "provider":
          return providerName(providerOf(m.id));
        case "input":
          return f?.cost?.input ?? null;
        case "output":
          return f?.cost?.output ?? null;
        case "context":
          return f?.context ?? null;
        case "maxOutput":
          return f?.maxOutput ?? null;
        case "released":
          return f?.releaseDate ?? null;
      }
    };
    const sign = sort.dir === "asc" ? 1 : -1;
    // Unknowns sink in both directions; ties fall back to the id.
    return [...filtered].sort((a, b) => {
      const va = value(a);
      const vb = value(b);
      if (va === null || vb === null) {
        if (va !== vb) return va === null ? 1 : -1;
      } else if (va !== vb) {
        const c =
          typeof va === "number" && typeof vb === "number"
            ? va - vb
            : String(va).localeCompare(String(vb));
        if (c !== 0) return sign * c;
      }
      return a.id.localeCompare(b.id);
    });
  }, [filtered, sort, providerName]);

  const { page, pageCount, setPage, pageRows } = usePagedRows(sorted, PAGE_SIZE);
  useEffect(() => setPage(1), [query, providerSet, makerSet, picked, sort, setPage]);

  const onSort = (key: SortKey) =>
    setSort((s) => (s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: "asc" }));

  // Counted over every filter but status, so picking *Hidden* still reports
  // how many are offered. Blocked rows stay listed but are never offered.
  const inScope = rows.filter((m) => matches(m, "status"));
  const blocked = inScope.filter((m) => blockedBecause(m) !== null).length;
  const hiddenCount = inScope.filter(
    (m) => blockedBecause(m) === null && isHidden(catalogue, m.id),
  ).length;
  const offered = Math.max(inScope.length - blocked - hiddenCount, 0);
  const status =
    blocked > 0
      ? `${offered} of ${inScope.length} offered · ${blocked} cannot run here`
      : `${offered} of ${inScope.length} offered`;

  // The header tick acts on the filtered rows only, across every page.
  const tickable = filtered.filter((m) => blockedBecause(m) === null);
  const showable = tickable.filter((m) => isHidden(catalogue, m.id)).map((m) => m.id);
  const hideable = tickable.filter((m) => !isHidden(catalogue, m.id)).map((m) => m.id);

  const clearFilters = () => {
    setProviderSet(new Set());
    setMakerSet(new Set());
    setPicked(null);
  };

  return {
    query,
    setQuery,
    providerSet,
    setProviderSet,
    makerSet,
    setMakerSet,
    providerOptions,
    makerOptions,
    makerLabel,
    clearFilters,
    sort,
    picked,
    scope,
    providerName,
    rows,
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
  };
}

function focusSet(focus: ModelsFocus): Set<string> {
  return new Set(focus.provider ? [focus.provider] : []);
}

/** One option per value any row has, counted under the other filters, by name. */
function facetOptions(
  rows: HarnessModel[],
  key: (m: HarnessModel) => string,
  label: (value: string) => string,
  counts: (m: HarnessModel) => boolean,
): FilterOption[] {
  const tally = new Map<string, number>();
  for (const m of rows) {
    const k = key(m);
    tally.set(k, (tally.get(k) ?? 0) + (counts(m) ? 1 : 0));
  }
  return [...tally]
    .map(([value, count]) => ({ value, label: label(value), count }))
    .sort((a, b) => a.label.localeCompare(b.label));
}
