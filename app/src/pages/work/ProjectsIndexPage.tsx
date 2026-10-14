import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { NewProjectButton } from "@/components/projects/nav/NewProjectButton";
import { SectionHeader } from "@/components/projects/page/SectionHeader";
import { ProjectsIndexTable } from "@/components/projects/lists/ProjectsIndexTable";
import { TaskFilters } from "@/components/projects/tasks/TaskFilters";
import { SubjectFilter } from "@/components/projects/tasks/SubjectFilter";
import { useTaskFilterState } from "@/components/projects/tasks/useTaskFilterState";
import { isFiltered } from "@/components/projects/tasks/filterRules";
import { matchesProjectFilter, projectSubjectCounts } from "@/components/projects/lists/projectFilters";
import { projectTableData } from "@/components/projects/lists/projectTableData";
import { projectHref } from "@/components/projects/nav/projectHref";
import { useProjectActions } from "@/components/projects/lists/useProjectActions";
import { TaskSearch } from "@/components/projects/tasks/TaskSearch";
import { useTaskActions } from "@/components/projects/tasks/useTaskActions";
import type { TaskSort } from "@/components/projects/tasks/taskSort";
import { useSubjects } from "@/hooks/data/useSubjects";
import { useTaskList } from "@/hooks/data/useTaskList";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import { useStoredSet } from "@/hooks/ui/useStoredState";
import { useProjectsStore } from "@/stores/planning/projectsStore";

/** Projects and their expandable task trees share one full-width table. */
export default function ProjectsIndexPage() {
  const { subjects } = useSubjects();
  const navigate = useNavigate();
  const { tasks, projects, projectById, loaded } = useTaskList("all");
  const { tags, byId: tagById, loaded: tagsLoaded } = useTaskTags();
  const createProject = useProjectsStore(s => s.createProject);
  const [pickedFilter, setFilter] = useTaskFilterState();
  const filter = useMemo(() => tagsLoaded
    ? { ...pickedFilter, tags: pickedFilter.tags.filter(id => tagById.has(id)) } : pickedFilter,
    [pickedFilter, tagsLoaded, tagById]);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<TaskSort>({ key: "due", dir: "asc" });
  const [expanded, setExpanded] = useStoredSet("oculus-projects-expanded");
  const searched = useMemo(() => projectTableData(projects, tasks, query, sort, id => tagById.get(id)?.name), [projects, tasks, query, sort, tagById]);
  const { branches, counts } = useMemo(() => {
    const now = Date.now();
    // Status uses the full tree even when search keeps only a matching subtask.
    const full = new Map(projectTableData(projects, tasks, "", sort, id => tagById.get(id)?.name)
      .map(branch => [branch.project.id, branch]));
    const candidates = searched.map(branch => full.get(branch.project.id)!);
    const matching = new Set(candidates.filter(b => matchesProjectFilter(b, filter, tagById, projectById, now)).map(b => b.project.id));
    return { branches: searched.filter(b => matching.has(b.project.id)),
      counts: projectSubjectCounts(candidates, filter, tagById, projectById, now) };
  }, [searched, projects, tasks, sort, filter, tagById, projectById]);
  const actions = useProjectActions();
  const taskActions = useTaskActions(tasks);
  const toggle = (id: number) => setExpanded(prev => {
    const next = new Set(prev);
    if (!next.delete(String(id))) next.add(String(id));
    return next;
  });
  const allExpanded = branches.length > 0 && branches.every(b => expanded.has(String(b.project.id)));
  const create = useCallback((subjectId: number | null, name: string) => {
    createProject({ name, subjectId }).then(id => navigate(projectHref({id, name})))
      .catch(e => console.error("create project failed", e));
  }, [createProject, navigate]);
  return <div className="flex h-full min-h-0 flex-col">
    <SectionHeader title="Projects">
      <span className="flex-1" />
      <span className="shrink-0 text-[12px] tabular-nums text-muted-foreground">{branches.length} {branches.length === 1 ? "project" : "projects"}</span>
      <NewProjectButton subjects={subjects} onCreate={create} />
    </SectionHeader>
    <div className="flex h-11 shrink-0 items-center gap-2 px-5">
      <TaskSearch value={query} onChange={setQuery} placeholder="Search projects and tasks" />
      <span className="flex-1" />
      <TaskFilters filter={filter} tags={tags} onChange={setFilter} target="projects" />
      <SubjectFilter filter={filter} counts={counts} onChange={setFilter} />
      <Button variant="ghost" size="sm" disabled={!branches.length || Boolean(query.trim())}
        onClick={() => setExpanded(prev => {
          const next = new Set(prev);
          for (const {project} of branches) { if (allExpanded) next.delete(String(project.id)); else next.add(String(project.id)); }
          return next;
        })}>{allExpanded ? "Collapse all" : "Expand all"}</Button>
    </div>
    <div className="min-h-0 flex-1">
      {!loaded ? <p className="px-6 py-16 text-center text-xs text-muted-foreground">Loading…</p> :
        <ProjectsIndexTable branches={branches} tasks={tasks} projects={projects} projectById={projectById}
          tags={tags} tagById={tagById} expanded={expanded} onToggle={toggle} query={query}
          empty={isFiltered(filter) ? "No projects match — change the search or filters." : undefined}
          sort={sort} onSort={setSort} actions={actions} taskActions={taskActions} />}
    </div>
  </div>;
}
