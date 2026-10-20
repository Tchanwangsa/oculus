import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { TasksTable } from "@/components/projects/lists/TasksTable";
import { DEFAULT_DISPLAY } from "@/components/projects/display/settings";
import { NO_PROJECTS, tag, task } from "./fixtures";

test("status edits live beside the name and progress includes hidden completed subtasks", () => {
  const parent = task({ id: 900, title: "Parent" });
  const child = task({ id: 901, parent_id: 900, title: "Visible child" });
  const done = task({ id: 902, parent_id: 900, title: "Hidden completed child", done_at: "2026-10-01 00:00:00", column_id: "done" });
  const html = renderToStaticMarkup(
    <MemoryRouter>
      <TasksTable
        tasks={[parent, child]}
        allTasks={[parent, child, done]}
        projects={[]}
        projectById={NO_PROJECTS}
        tags={[]}
        tagById={new Map()}
        display={{ ...DEFAULT_DISPLAY, groupSubtasks: true }}
        onSort={() => {}}
        empty={false}
        actions={{ onMove: () => {}, onTags: () => {}, onRefile: () => {}, onCreateTag: async (name) => tag(1, name) }}
      />
    </MemoryRouter>,
  );
  expect(html).toContain('aria-label="Change status"');
  expect(html).not.toContain('aria-label="Sort by Status"');
  expect(html).toContain('aria-label="1 of 2 subtasks complete; collapse subtasks"');
  expect(html).toContain('stroke-dasharray="50 100"');
  expect(html).toContain("Visible child");
  expect(html).not.toContain("Hidden completed child");
});
