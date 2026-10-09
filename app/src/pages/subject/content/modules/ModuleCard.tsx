import { CaretDown, CaretRight } from "@phosphor-icons/react";
import type { LoadedModule } from "@/hooks/data/useModuleTocs";
import type { DbFile } from "@/lib/db";
import { ItemRow } from "./ItemRow";
import type { SubjectRef } from "./types";

export function ModuleCard({
  module: mod, subject, files, open, onToggle,
}: {
  module: LoadedModule;
  subject: SubjectRef;
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
                  subject={subject}
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
