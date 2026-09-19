import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowsClockwise, WarningCircle } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { getDb, getSubjects } from "@/lib/db";
import { fmtBytes, displayCode } from "@/lib/format";

interface StorageFile {
  path: string;
  bytes: number;
}

interface StorageReport {
  data_dir: string;
  total_bytes: number;
  disk_free_bytes: number;
  disk_total_bytes: number;
  files: StorageFile[];
}

interface Group {
  label: string;
  bytes: number;
  /** Tailwind bg-* class carrying the series color. */
  color: string;
}

const LIMIT_KEY = "oculus-storage-limit-gb";
const LIMIT_OPTIONS = [1, 2, 5, 10, 25, 50, 100];
const GB = 1024 ** 3;

/** Series slots in validated order (see index.css) + the de-emphasis gray. */
const SLOTS = ["bg-chart-1", "bg-chart-2", "bg-chart-3", "bg-chart-4", "bg-chart-5"];
const OTHER_COLOR = "bg-chart-other";

/** File-type entities keep a fixed color regardless of their current rank —
 *  color follows the entity, never its size. */
const TYPE_COLOR: Record<string, string> = {
  PDFs: "bg-chart-1",
  Videos: "bg-chart-2",
  Images: "bg-chart-3",
  Markdown: "bg-chart-4",
  Database: "bg-chart-5",
  Other: OTHER_COLOR,
};

function typeOf(path: string): string {
  const name = (path.split("/").pop() ?? path).toLowerCase();
  if (name.endsWith(".pdf")) return "PDFs";
  if (/\.(mp4|m4v|mov|webm)$/.test(name)) return "Videos";
  if (/\.(png|jpe?g|gif|webp|svg|bmp)$/.test(name)) return "Images";
  if (name.endsWith(".md")) return "Markdown";
  if (name.startsWith("oculus.db")) return "Database";
  return "Other";
}

/** Mirror of Rust's `paths::safe_dir` — how a subject code becomes its
 *  `courses/<dir>` folder name. */
function safeDir(s: string): string {
  return [...s].map((c) => (/[\p{L}\p{N}_-]/u.test(c) ? c : "_")).join("");
}

export function StorageSection() {
  const [report, setReport] = useState<StorageReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [groupBy, setGroupBy] = useState<"type" | "subject">("type");
  const [subjectByDir, setSubjectByDir] = useState<Map<string, string>>(new Map());
  const [subjectByLecture, setSubjectByLecture] = useState<Map<string, string>>(new Map());
  const [limitGb, setLimitGb] = useState<number | null>(() => {
    const v = Number(localStorage.getItem(LIMIT_KEY));
    return Number.isFinite(v) && v > 0 ? v : null;
  });

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [rep, subjects] = await Promise.all([
        invoke<StorageReport>("storage_report"),
        getSubjects(),
      ]);
      setReport(rep);

      const byDir = new Map<string, string>();
      const byId = new Map<number, string>();
      for (const s of subjects) {
        byDir.set(safeDir(s.code), displayCode(s.code));
        byId.set(s.id, displayCode(s.code));
      }
      setSubjectByDir(byDir);

      // Attribution nicety only — a missing lectures table shouldn't take
      // the whole storage view down with it.
      try {
        const db = await getDb();
        const lectures = await db.select<{ id: string; subject_id: number }[]>(
          "SELECT id, subject_id FROM lectures",
        );
        setSubjectByLecture(
          new Map(
            lectures.map((l) => [l.id, byId.get(l.subject_id) ?? "Lectures"]),
          ),
        );
      } catch {
        setSubjectByLecture(new Map());
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const setLimit = (gb: number | null) => {
    setLimitGb(gb);
    if (gb == null) localStorage.removeItem(LIMIT_KEY);
    else localStorage.setItem(LIMIT_KEY, String(gb));
  };

  // ── Grouping ──────────────────────────────────────────────────────────────

  const groups: Group[] = useMemo(() => {
    if (!report) return [];
    const sums = new Map<string, number>();

    for (const f of report.files) {
      let key: string;
      if (groupBy === "type") {
        key = typeOf(f.path);
      } else {
        const [top, second] = f.path.split("/");
        if (top === "courses" && second) {
          key = subjectByDir.get(second) ?? displayCode(second);
        } else if (top === "lectures" && second) {
          key = subjectByLecture.get(second) ?? "Lectures";
        } else {
          key = "App data";
        }
      }
      sums.set(key, (sums.get(key) ?? 0) + f.bytes);
    }

    const ranked = [...sums.entries()].sort((a, b) => b[1] - a[1]);

    if (groupBy === "type") {
      // Fixed entity→color mapping; "Other" always wears the gray.
      return ranked.map(([label, bytes]) => ({
        label,
        bytes,
        color: TYPE_COLOR[label] ?? OTHER_COLOR,
      }));
    }

    // Subjects: at most 5 colored series, tail folded into a gray "Other".
    // Slots are handed out alphabetically among the kept entities so a
    // subject keeps its color as sizes drift between visits.
    const kept = ranked.slice(0, SLOTS.length);
    const tail = ranked.slice(SLOTS.length);
    const slotByLabel = new Map(
      kept
        .map(([label]) => label)
        .sort((a, b) => a.localeCompare(b))
        .map((label, i) => [label, SLOTS[i]]),
    );
    const out: Group[] = kept.map(([label, bytes]) => ({
      label,
      bytes,
      color: slotByLabel.get(label)!,
    }));
    if (tail.length > 0) {
      out.push({
        label: "Other",
        bytes: tail.reduce((n, [, b]) => n + b, 0),
        color: OTHER_COLOR,
      });
    }
    return out;
  }, [report, groupBy, subjectByDir, subjectByLecture]);

  const largest = useMemo(() => report?.files.slice(0, 6) ?? [], [report]);

  // ── Meter geometry ────────────────────────────────────────────────────────

  const used = report?.total_bytes ?? 0;
  const limitBytes = limitGb != null ? limitGb * GB : null;
  const overLimit = limitBytes != null && used > limitBytes;
  // 100% of the bar = the limit when one is set (used vs capacity); with no
  // limit the bar shows pure composition of what's used. When over the limit
  // the scale grows to fit and a marker shows where the limit sits.
  const scale = limitBytes != null ? Math.max(used, limitBytes) : used;

  if (error) {
    return (
      <p className="text-xs text-destructive py-2">
        Storage scan failed: {error}
      </p>
    );
  }

  return (
    <div>
      {/* Headline + grouping toggle */}
      <div className="flex items-end justify-between mb-3">
        <div>
          <p className="text-sm font-semibold text-foreground tabular-nums leading-none">
            {report ? fmtBytes(used) : "—"}
            <span className="text-xs font-normal text-muted-foreground ml-1.5">
              used
            </span>
          </p>
          <p className="text-xs text-muted-foreground mt-1.5">
            {limitBytes != null && (
              <span className={cn(overLimit && "text-destructive")}>
                {overLimit
                  ? `${fmtBytes(used - limitBytes)} over the ${limitGb} GB limit`
                  : `${fmtBytes(limitBytes - used)} left of ${limitGb} GB limit`}
                {" · "}
              </span>
            )}
            {report ? `${fmtBytes(report.disk_free_bytes)} free on disk` : ""}
          </p>
        </div>

        <div className="flex items-center gap-1">
          <ToggleGroup
            type="single"
            size="sm"
            value={groupBy}
            onValueChange={(v) => v && setGroupBy(v as "type" | "subject")}
            className="bg-surface rounded-md p-0.5"
          >
            <ToggleGroupItem
              value="type"
              className="h-6 px-2.5 text-xs rounded-[5px] data-[state=on]:bg-card data-[state=on]:shadow-xs"
            >
              File type
            </ToggleGroupItem>
            <ToggleGroupItem
              value="subject"
              className="h-6 px-2.5 text-xs rounded-[5px] data-[state=on]:bg-card data-[state=on]:shadow-xs"
            >
              Subject
            </ToggleGroupItem>
          </ToggleGroup>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Rescan storage"
                onClick={load}
                disabled={loading}
                className="text-muted-foreground/60"
              >
                <ArrowsClockwise size={13} className={cn(loading && "animate-spin")} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Rescan storage</TooltipContent>
          </Tooltip>
        </div>
      </div>

      {/* Meter — stacked usage bar on a capacity track */}
      <div className="relative h-2.5 rounded-full bg-surface overflow-hidden">
        <div className="absolute inset-0 flex gap-[2px]">
          {groups
            .filter((g) => g.bytes > 0)
            .map((g) => (
              <Tooltip key={g.label}>
                <TooltipTrigger asChild>
                  <div
                    className={cn("h-full", g.color)}
                    style={{
                      width: `${scale ? (g.bytes / scale) * 100 : 0}%`,
                      minWidth: 4,
                    }}
                  />
                </TooltipTrigger>
                <TooltipContent>
                  {g.label} — {fmtBytes(g.bytes)}
                  {used > 0 ? ` (${Math.round((g.bytes / used) * 100)}%)` : ""}
                </TooltipContent>
              </Tooltip>
            ))}
        </div>
        {/* Limit marker, only needed once usage has pushed past it. */}
        {overLimit && limitBytes != null && (
          <div
            className="absolute top-0 h-full w-[2px] bg-destructive"
            style={{ left: `${(limitBytes / scale) * 100}%` }}
          />
        )}
      </div>

      {overLimit && (
        <p className="flex items-center gap-1.5 text-xs text-destructive mt-2">
          <WarningCircle size={13} />
          Over the storage limit — clear lecture videos or lower what you sync.
        </p>
      )}

      {/* Breakdown */}
      <div className="mt-3">
        {groups.map((g) => (
          <div key={g.label} className="flex items-center gap-2.5 py-1.5">
            <span className={cn("w-2.5 h-2.5 rounded-[3px] shrink-0", g.color)} />
            <span className="text-xs text-foreground flex-1 min-w-0 truncate">
              {g.label}
            </span>
            <span className="text-xs text-muted-foreground tabular-nums">
              {used > 0 ? `${Math.round((g.bytes / used) * 100)}%` : "—"}
            </span>
            <span className="text-xs text-foreground tabular-nums w-18 text-right">
              {fmtBytes(g.bytes)}
            </span>
          </div>
        ))}
      </div>

      {/* Largest files */}
      {largest.length > 0 && (
        <div className="mt-5">
          <p className="font-display text-[13px] font-semibold text-foreground mb-1.5">
            Largest files
          </p>
          <div>
            {largest.map((f) => {
              const parts = f.path.split("/");
              const name = parts[parts.length - 1];
              const where =
                parts[0] === "courses" && parts[1]
                  ? (subjectByDir.get(parts[1]) ?? displayCode(parts[1]))
                  : parts[0] === "lectures" && parts[1]
                    ? (subjectByLecture.get(parts[1]) ?? "Lectures")
                    : "App data";
              return (
                <div key={f.path} className="flex items-center gap-2.5 py-1.5">
                  <span
                    className="text-xs text-foreground flex-1 min-w-0 truncate"
                    title={f.path}
                  >
                    {name}
                  </span>
                  <span className="text-xs text-muted-foreground shrink-0">
                    {where}
                  </span>
                  <span className="text-xs text-foreground tabular-nums w-18 text-right shrink-0">
                    {fmtBytes(f.bytes)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Limit setting */}
      <div className="flex items-center justify-between gap-4 mt-6">
        <div>
          <p className="text-xs font-medium text-foreground">Storage limit</p>
          <p className="text-xs text-muted-foreground mt-0.5">
            Warns when Oculus's data goes past this size.
          </p>
        </div>
        <Select
          value={limitGb != null ? String(limitGb) : "none"}
          onValueChange={(v) => setLimit(v === "none" ? null : Number(v))}
        >
          <SelectTrigger size="sm" className="w-28 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">No limit</SelectItem>
            {LIMIT_OPTIONS.map((gb) => (
              <SelectItem key={gb} value={String(gb)}>
                {gb} GB
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}
