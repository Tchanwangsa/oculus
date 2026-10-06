import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Brain,
  CircleNotch,
  FilePdf,
  Image as ImageIcon,
  Paperclip,
  VideoCamera,
  Waveform,
  type Icon,
} from "@phosphor-icons/react";

import type { OpencodeProvider } from "@/lib/opencodeAuth";
import { providerOf, type OpencodeCatalogue } from "@/lib/opencodeCatalogue";
import type { HarnessModel, ModelFacts } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { GridTable } from "@/components/ui/GridTable";
import { PillTabs } from "@/components/ui/PillTabs";
import { usePagedRows } from "@/components/ui/TablePagination";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { blockedBecause, isHidden } from "./useOpencode";
import { SearchField, SortHeader, TableEmpty, type SortDir } from "./parts";

/** Column template shared by the header and every row; the model takes the slack. */
const COLS =
  "grid grid-cols-[16px_minmax(0,1fr)_120px_64px_68px_56px_68px_96px_64px] items-center gap-3 px-5";

const PAGE_SIZE = 50;

const ALL = "all";

type Scope = "all" | "offered" | "hidden";
type SortKey = "id" | "provider" | "input" | "output" | "context" | "maxOutput" | "released";

/** Where the Providers tab sends the table: a provider to filter to (or none),
 *  and a counter so the same provider twice still resets the filters. */
export interface ModelsFocus {
  provider: string | null;
  n: number;
}

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

function ModelRow({
  model,
  provider,
  hidden,
  onHidden,
}: {
  model: HarnessModel;
  provider: string;
  hidden: boolean;
  onHidden: (hidden: boolean) => void;
}) {
  // A model the gate never offers can't be ticked.
  const why = blockedBecause(model);
  const blocked = why !== null;
  const f = model.facts;

  return (
    <div className={cn(COLS, "py-2 transition-colors hover:bg-surface/60")}>
      <Checkbox
        aria-label={`Offer ${model.label}`}
        checked={!blocked && !hidden}
        disabled={blocked}
        onCheckedChange={(v) => onHidden(v !== true)}
      />

      <div className="min-w-0">
        <div className={cn("truncate text-xs", blocked ? "text-muted-foreground" : "text-foreground")}>
          {model.label}
        </div>
        <div className="truncate text-[11px] text-muted-foreground">{model.id}</div>
        {why && <div className="text-[11px] text-muted-foreground">{why}</div>}
      </div>

      <span className="truncate text-xs text-muted-foreground">{provider}</span>

      <Num>{fmtPrice(f?.cost?.input)}</Num>
      <Num>{fmtPrice(f?.cost?.output)}</Num>
      <Num>{fmtTokens(f?.context)}</Num>
      <Num>{fmtTokens(f?.maxOutput)}</Num>
      <Capabilities facts={f} />
      <Num>{fmtRelease(f?.releaseDate)}</Num>
    </div>
  );
}

function Num({ children }: { children: string }) {
  return (
    <span
      className={cn(
        "justify-self-end text-xs tabular-nums",
        children === "—" ? "text-muted-foreground/60" : "text-foreground",
      )}
    >
      {children}
    </span>
  );
}

const INPUTS: ReadonlyArray<{ key: string; label: string; icon: Icon }> = [
  { key: "image", label: "Takes images", icon: ImageIcon },
  { key: "pdf", label: "Takes PDFs", icon: FilePdf },
  { key: "audio", label: "Takes audio", icon: Waveform },
  { key: "video", label: "Takes video", icon: VideoCamera },
];

/** What the model can do beyond text; tool calling is a block reason, not an icon. */
function Capabilities({ facts }: { facts: ModelFacts | undefined }) {
  if (!facts) return <span className="text-xs text-muted-foreground/60">—</span>;
  const marks: Array<{ label: string; icon: Icon }> = [];
  if (facts.reasoning) marks.push({ label: "Reasons", icon: Brain });
  for (const input of INPUTS) if (facts.inputs.includes(input.key)) marks.push(input);
  if (facts.attachment) marks.push({ label: "Takes attachments", icon: Paperclip });
  if (marks.length === 0) return <span className="text-xs text-muted-foreground/60">—</span>;
  return (
    <div className="flex items-center gap-1.5 text-muted-foreground">
      {marks.map(({ label, icon: Mark }) => (
        <Tooltip key={label}>
          <TooltipTrigger asChild>
            <span aria-label={label} className="flex">
              <Mark size={13} />
            </span>
          </TooltipTrigger>
          <TooltipContent>{label}</TooltipContent>
        </Tooltip>
      ))}
    </div>
  );
}

// ── facts formatting: null is unknown ("—"), never $0 ─────────────────────

/** USD per million tokens: `$0`, `$0.15`, `$3`, `$2.50`. */
function fmtPrice(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n === 0) return "$0";
  if (Number.isInteger(n)) return `$${n}`;
  if (n >= 1) return `$${n.toFixed(2)}`;
  if (n < 0.01) return `$${Number(n.toPrecision(2))}`;
  return `$${n.toFixed(3).replace(/0$/, "")}`;
}

/** A token count: `128K`, `1M`, `1.5M`. */
function fmtTokens(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n >= 1_000_000) return `${Number((n / 1_000_000).toFixed(1))}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`;
  return String(n);
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** `YYYY-MM-DD` (or `YYYY-MM`) as `Aug 2026`. */
function fmtRelease(date: string | null | undefined): string {
  const m = date?.match(/^(\d{4})-(\d{2})/);
  const month = m ? MONTHS[Number(m[2]) - 1] : undefined;
  return m && month ? `${month} ${m[1]}` : "—";
}
