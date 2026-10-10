import { cn } from "@/lib/utils";
import type { SyncRunSummary } from "@/lib/db";

export function FileCounts({ run }: { run: SyncRunSummary }) {
  if (run.file_count === 0) {
    return <span className="text-[11px] text-muted-foreground/60">—</span>;
  }
  const parts: Array<{ n: number; label: string; cls: string }> = [
    { n: run.new_count, label: "downloaded", cls: "text-success" },
    { n: run.updated_count, label: "updated", cls: "text-brand" },
    { n: run.unchanged_count, label: "skipped", cls: "text-muted-foreground" },
  ];
  return (
    <span className="text-[11px] text-muted-foreground truncate">
      {parts
        .filter((p) => p.n > 0)
        .map((p, i) => (
          <span key={p.label}>
            {i > 0 && <span className="text-muted-foreground/40"> · </span>}
            <span className={cn("tabular-nums", p.cls)}>{p.n}</span> {p.label}
          </span>
        ))}
    </span>
  );
}
