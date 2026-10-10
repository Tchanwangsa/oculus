import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Stack } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useModuleTocs } from "@/hooks/data/useModuleTocs";
import { useSubject } from "@/layouts/SubjectLayout";
import { ModuleCard } from "./modules/ModuleCard";

/**
 * The Canvas modules page, rebuilt: one collapsible card per module, its items
 * grouped under the SubHeaders Canvas puts between them. Rows open in the
 * side panel.
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
    return <SubjectLoading count={3} rowClassName="h-28" spacing="space-y-3" />;
  }

  if (modules.length === 0) {
    return (
      <SubjectEmpty icon={<Stack size={24} className="text-muted-foreground/40" />} title="No modules scraped yet.">
        <Button
          variant="link"
          className="h-auto p-0 text-xs font-normal"
          onClick={() => navigate("/sync")}
        >
          Run a sync →
        </Button>
      </SubjectEmpty>
    );
  }

  return (
    <SubjectPage>
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
            subject={subject}
            files={files}
            open={!collapsed.has(mod.relPath)}
            onToggle={() => toggle(mod.relPath)}
          />
        ))}
      </div>
    </SubjectPage>
  );
}
