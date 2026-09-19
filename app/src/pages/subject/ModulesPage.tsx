import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  ArrowSquareOut,
  CaretDown,
  CaretRight,
  FileText,
  PencilLine,
  Rocket,
  Stack,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useModuleTocs, type LoadedModule } from "@/hooks/useModuleTocs";
import { useSubject } from "@/layouts/SubjectLayout";
import { filePageHref, openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { fileIconFor } from "@/lib/fileTypes";
import { resolveTocHref, type ModuleItem } from "@/lib/moduleToc";
import type { DbFile } from "@/lib/db";

/**
 * The Canvas modules page, rebuilt: one collapsible card per module, its items
 * grouped under the SubHeaders Canvas puts between them. Rows open in the peek.
 */
export default function SubjectModulesPage() {
  const subject = useSubject();
  const navigate = useNavigate();
  const { files, byCategory, loading: filesLoading } = useSubjectFiles(subject.id);
  const modules = useModuleTocs(byCategory.module, filesLoading);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const toggle = (relPath: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(relPath)) next.add(relPath);
      return next;
    });

  const allCollapsed = useMemo(
    () => modules != null && modules.length > 0 && collapsed.size === modules.length,
    [collapsed, modules],
  );

  if (modules == null) {
    return (
      <div className="page-scroll">
        <div className="mx-auto max-w-5xl px-6 py-6 space-y-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <Skeleton key={i} className="h-28 w-full" />
          ))}
        </div>
      </div>
    );
  }

  if (modules.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2">
        <Stack size={24} className="text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">No modules scraped yet.</p>
        <Button
          variant="link"
          className="h-auto p-0 text-xs font-normal"
          onClick={() => navigate("/sync")}
        >
          Run a sync →
        </Button>
      </div>
    );
  }

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-5">
        <div className="mb-3 flex justify-end">
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-[11px] text-muted-foreground"
            onClick={() =>
              setCollapsed(
                allCollapsed ? new Set() : new Set(modules.map((m) => m.relPath)),
              )
            }
          >
            {allCollapsed ? "Expand all" : "Collapse all"}
          </Button>
        </div>

        <div className="space-y-2.5">
          {modules.map((mod) => (
            <ModuleCard
              key={mod.relPath}
              module={mod}
              files={files}
              open={!collapsed.has(mod.relPath)}
              onToggle={() => toggle(mod.relPath)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Pieces ────────────────────────────────────────────────────────────────────

function ModuleCard({
  module: mod, files, open, onToggle,
}: {
  module: LoadedModule;
  files: DbFile[];
  open: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="rounded-lg border border-border overflow-hidden">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-3 py-2.5 bg-surface hover:bg-surface-raised transition-colors text-left"
      >
        {open ? (
          <CaretDown size={11} className="text-muted-foreground shrink-0" />
        ) : (
          <CaretRight size={11} className="text-muted-foreground shrink-0" />
        )}
        <span className="text-[12px] font-semibold text-foreground truncate flex-1">
          {mod.title}
        </span>
      </button>

      {open && (
        <div className="divide-y divide-border-subtle">
          {mod.sections.map((section, i) => (
            <div key={i}>
              {section.heading && (
                <div className="px-3 pt-2.5 pb-1">
                  <span className="font-display text-[11px] font-semibold text-muted-foreground">
                    {section.heading}
                  </span>
                </div>
              )}
              {section.items.map((item, j) => (
                <ItemRow
                  key={j}
                  item={item}
                  files={files}
                  moduleRelPath={mod.relPath}
                />
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function itemIcon(item: ModuleItem, target: DbFile | null) {
  if (item.kind === "quiz") return Rocket;
  if (item.kind === "assignment") return PencilLine;
  if (item.kind === "external") return ArrowSquareOut;
  if (target?.category === "file") return fileIconFor(target.filename);
  // Everything else is (or links to) a Canvas page.
  return FileText;
}

function ItemRow({
  item, files, moduleRelPath,
}: {
  item: ModuleItem;
  files: DbFile[];
  moduleRelPath: string;
}) {
  const resolved = item.href ? resolveTocHref(item.href, moduleRelPath) : null;
  const internalPath = resolved?.kind === "internal" ? resolved.path : null;
  const target = internalPath
    ? files.find((f) => f.relative_path === internalPath) ??
      // Module docs written before Office rows kept their original names link
      // to the converted PDF ("deck.pptx.pdf") — resolve those to the row.
      files.find((f) => internalPath === `${f.relative_path}.pdf`) ??
      null
    : null;
  const Icon = itemIcon(item, target);

  const inner = (
    <>
      <span className="shrink-0" style={{ width: item.indent * 14 }} />
      <Icon size={13} className="shrink-0 opacity-60" />
      <span className="text-[12px] truncate flex-1">{item.title}</span>
      {target && <FileRecency file={target} />}
    </>
  );

  const rowClass =
    "w-full flex items-center gap-2.5 px-3 py-2 text-left transition-colors";

  if (target) {
    return (
      <button
        data-tab-href={filePageHref(target) ?? undefined}
        onClick={() => openFileSmart(target)}
        className={cn(rowClass, "text-foreground hover:bg-surface")}
      >
        {inner}
      </button>
    );
  }

  if (resolved?.kind === "external") {
    return (
      <a
        href={resolved.url}
        target="_blank"
        rel="noreferrer"
        className={cn(rowClass, "text-foreground hover:bg-surface")}
      >
        {inner}
      </a>
    );
  }

  // No target: Canvas listed it, but the scraper couldn't download it (locked
  // file, unsupported type). Shown so the module isn't silently incomplete.
  return (
    <div className={cn(rowClass, "text-muted-foreground/70 cursor-default")}>
      {inner}
    </div>
  );
}
