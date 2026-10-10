import { useMemo } from "react";
import { ArrowSquareOut, PencilLine, Rocket } from "@phosphor-icons/react";
import { SubjectEmpty, SubjectPage } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useModuleTocs } from "@/hooks/data/useModuleTocs";
import { useSubject } from "@/layouts/SubjectLayout";
import { ListCard } from "@/components/ui/layout/PageParts";

interface TocTask {
  title: string;
  kind: "quiz" | "assignment";
  moduleTitle: string;
  /** Canvas URL — nothing local to open, so rows go to the browser. */
  url: string | null;
}

/** The old module-TOC listing, shown until a sync writes real documents. */
export function TocFallback() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);
  const modules = useModuleTocs(byCategory.module, loading);

  const tasks = useMemo<TocTask[]>(() => {
    if (!modules) return [];
    const out: TocTask[] = [];
    for (const mod of modules) {
      for (const section of mod.sections) {
        for (const item of section.items) {
          if (item.kind === "quiz" || item.kind === "assignment") {
            out.push({
              title: item.title,
              kind: item.kind,
              moduleTitle: mod.title,
              url: item.href && /^https?:/i.test(item.href) ? item.href : null,
            });
          }
        }
      }
    }
    return out;
  }, [modules]);

  if (tasks.length === 0) {
    return (
      <SubjectEmpty icon={<PencilLine size={24} className="text-muted-foreground/40" />} title="No quizzes or assignments scraped yet — run a sync." />
    );
  }

  return (
    <SubjectPage>
      <ListCard>
        {tasks.map((t, i) => {
          const Icon = t.kind === "quiz" ? Rocket : PencilLine;
          const inner = (
            <>
              <Icon size={13} className="shrink-0 opacity-60" />
              <span className="min-w-0 flex-1">
                <span className="block text-[12px] text-foreground truncate">
                  {t.title}
                </span>
                <span className="block text-[11px] text-muted-foreground truncate">
                  {t.moduleTitle}
                </span>
              </span>
              <span className="shrink-0 text-[10px] uppercase tracking-wide text-muted-foreground">
                {t.kind}
              </span>
              {t.url && (
                <ArrowSquareOut size={12} className="shrink-0 opacity-40" />
              )}
            </>
          );
          const rowClass =
            "w-full flex items-center gap-3 px-3 py-2.5 text-left transition-colors";
          return t.url ? (
            <a
              key={i}
              href={t.url}
              target="_blank"
              rel="noreferrer"
              className={`${rowClass} hover:bg-surface`}
            >
              {inner}
            </a>
          ) : (
            <div key={i} className={`${rowClass} cursor-default`}>
              {inner}
            </div>
          );
        })}
      </ListCard>
    </SubjectPage>
  );
}
