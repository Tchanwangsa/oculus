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
    <div className="border-b border-border-subtle last:border-b-0">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 py-2.5 text-left hover:text-foreground transition-colors"
      >
        {open ? (
          <CaretDown size={11} className="text-muted-foreground shrink-0" />
        ) : (
          <CaretRight size={11} className="text-muted-foreground shrink-0" />
        )}
        <span className="text-[13px] font-medium text-foreground truncate flex-1">
          {mod.title}
        </span>
      </button>

      {open && (
        <div className="pb-1">
          {mod.sections.map((section, i) => (
            <div key={i}>
              {section.heading && (
                <div className="pt-2 pb-1">
                  <span className="text-[11px] font-medium text-muted-foreground">
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
