import { z } from "zod/v4";
import type { Reads } from "./reads";
import { limit, subject, subjectIds, tool, projectEvent } from "./shared";

export function planningTools(read: Reads) {
  return {
    tasks: tool(z.object({
      project: z.number().int().positive().optional(), unfiled: z.boolean().optional(),
      column: z.string().min(1).max(200).optional(), tag: z.string().min(1).max(200).optional(),
      due_before: z.union([z.iso.date(), z.iso.datetime({ offset: true })]).optional(), limit,
    }).strict().refine(a => !(a.project && a.unfiled) && (!a.column || !!a.project), "column needs project; project and unfiled are exclusive"), [projectEvent], async a => {
      const project = a.project ? await read.getProject(a.project) : null;
      if (a.project && !project) throw new Error(`Unknown project: ${a.project}`);
      if (a.column && !project?.columns.some(c => c.id === a.column)) throw new Error(`Unknown column: ${a.column}`);
      let rows = a.project ? await read.getTasks(a.project) : await read.getAllTasks();
      if (a.unfiled) rows = rows.filter(t => t.project_id == null);
      if (a.column) rows = rows.filter(t => t.column_id === a.column);
      if (a.due_before) rows = rows.filter(t => t.due_at != null && t.due_at < a.due_before!);
      if (a.tag) {
        const tag = (await read.getTaskTags()).find(t => t.name.toLowerCase() === a.tag!.toLowerCase());
        if (!tag) throw new Error(`Unknown tag: ${a.tag}`);
        rows = rows.filter(t => t.tag_ids.includes(tag.id));
      }
      if (!a.project) rows.sort((a, b) => (a.project_id ?? -1) - (b.project_id ?? -1) || a.position - b.position || a.id - b.id);
      rows = rows.slice(0, a.limit);
      const files = await read.getFileCounts(rows.map(t => t.id));
      return rows.map(t => ({ file_count: files.get(t.id) ?? 0, id: t.id, project_id: t.project_id, parent_id: t.parent_id, title: t.title, body: t.body, column_id: t.column_id, position: t.position, starts_at: t.starts_at, due_at: t.due_at, estimate_minutes: t.estimate_minutes, done_at: t.done_at, tag_ids: t.tag_ids, source: t.source, created_at: t.created_at, updated_at: t.updated_at }));
    }),
    projects: tool(z.object({ subject, personal: z.boolean().optional(), archived: z.boolean().optional(), limit }).strict()
      .refine(a => !(a.personal && a.subject !== undefined), "personal and subject are exclusive"), [projectEvent], async a => {
      const ids = a.subject === undefined ? undefined : subjectIds(await read.getSubjects(), a.subject, true);
      const rows = (await read.getProjects({ subjectId: a.personal ? null : ids?.[0], status: a.archived ? "archived" : "active" })).slice(0, a.limit);
      const counts = await read.getTaskCounts(rows.map(p => p.id));
      return rows.map(p => ({ ...p, tasks_total: counts.get(p.id)?.total ?? 0, tasks_done: counts.get(p.id)?.done ?? 0 }));
    }),
  };
}
