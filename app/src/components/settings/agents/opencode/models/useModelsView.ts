import { useCallback, useEffect, useMemo, useState } from "react";
import type { OpencodeProvider } from "@/lib/harness/opencodeAuth";
import { providerOf, type OpencodeCatalogue } from "@/lib/harness/opencodeCatalogue";
import type { HarnessModel } from "@/lib/harness";
import { usePagedRows } from "@/components/ui/table/TablePagination";
import { blockedBecause, isHidden } from "../useOpencode";
import type { SortDir } from "../parts";
import { ALL, PAGE_SIZE, type ModelsFocus, type Scope, type SortKey } from "./constants";

/** The table's filters, sort, pagination and the counts drawn from them. */
export function useModelsView(
  models: HarnessModel[] | null,
  providers: OpencodeProvider[] | null,
  catalogue: OpencodeCatalogue | null,
  focus: ModelsFocus,
) {
  const [query, setQuery] = useState("");
  const [provider, setProvider] = useState<string>(focus.provider ?? ALL);
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
    setProvider(focus.provider ?? ALL);
  }, [focus.n, focus.provider]);

  const names = useMemo(
    () => new Map((providers ?? []).map((p) => [p.id, p.name])),
    [providers],
  );
  const providerName = useCallback((id: string) => names.get(id) ?? id, [names]);

  // A hidden provider's models never reach the picker, so they aren't listed.
  const rows = useMemo(() => {
    if (!models) return [];
    const hiddenIds = new Set(catalogue?.hiddenProviders ?? []);
    return models.filter((m) => !hiddenIds.has(providerOf(m.id)));
  }, [models, catalogue]);

  const providerIds = useMemo(
    () =>
      [...new Set(rows.map((m) => providerOf(m.id)))].sort((a, b) =>
        providerName(a).localeCompare(providerName(b)),
      ),
    [rows, providerName],
  );

  const inProvider = useMemo(
    () => (provider === ALL ? rows : rows.filter((m) => providerOf(m.id) === provider)),
    [rows, provider],
  );

  const onScope = (next: Scope) =>
    setPicked(
      next === "all"
        ? null
        : {
            scope: next,
            ids: new Set(
              rows
                .filter((m) => blockedBecause(m) === null)
                .filter((m) => isHidden(catalogue, m.id) === (next === "hidden"))
                .map((m) => m.id),
            ),
          },
    );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return inProvider.filter(
      (m) =>
        (!picked || picked.ids.has(m.id)) &&
        (!q || m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q)),
    );
  }, [inProvider, picked, query]);

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
  useEffect(() => setPage(1), [query, provider, picked, sort, setPage]);

  const onSort = (key: SortKey) =>
    setSort((s) => (s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: "asc" }));

  // Blocked rows stay listed with their reason but are never counted offered.
  const blocked = inProvider.filter((m) => blockedBecause(m) !== null).length;
  const hiddenCount = inProvider.filter(
    (m) => blockedBecause(m) === null && isHidden(catalogue, m.id),
  ).length;
  const offered = Math.max(inProvider.length - blocked - hiddenCount, 0);
  const status =
    blocked > 0
      ? `${offered} of ${inProvider.length} offered · ${blocked} cannot run here`
      : `${offered} of ${inProvider.length} offered`;

  // Bulk ticks act on the filtered rows only, across every page.
  const tickable = filtered.filter((m) => blockedBecause(m) === null);
  const showable = tickable.filter((m) => isHidden(catalogue, m.id)).map((m) => m.id);
  const hideable = tickable.filter((m) => !isHidden(catalogue, m.id)).map((m) => m.id);

  return {
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
  };
}
