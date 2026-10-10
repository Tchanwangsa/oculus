import { create } from "zustand";
import {
  archiveProject,
  createProject,
  createTask,
  deleteProject,
  deleteTask,
  getProject,
  getProjects,
  getTaskCounts,
  getTasks,
  moveTask,
  refileTask,
  unarchiveProject,
  updateProject,
  updateTask,
  type CreateProjectInput,
  type CreateTaskInput,
  type DbProject,
  type DbProjectTask,
  type GetProjectsOptions,
  type ProjectTaskCounts,
  type UpdateProjectInput,
  type UpdateTaskInput,
} from "@/lib/planning/projects";

/**
 * The project list and the open project's tasks; I/O goes through
 * `app/src/lib/planning/projects/`. No Tauri `listen()` here — `useBackendEvents` is
 * the one bridge. **A write does not re-read**: `PROJECTS_UPDATED_EVENT` is the
 * only refresh path, so UI and CLI writes arrive by the same door and a page
 * listening for it calls {@link reload}.
 */
interface ProjectsState {
  projects: DbProject[];
  /** What {@link projects} is filtered by; `loadProjects()` with no argument
   *  re-runs it, so the index is `loadProjects({})`. */
  query: GetProjectsOptions;
  /** Finished/total per project id, read with the list in one grouped query.
   *  Every id has an entry (0/0 when empty), so a missing key is a bug. */
  counts: Map<number, ProjectTaskCounts>;
  /** The project whose board is on screen; `null` is the list. */
  activeId: number | null;
  /** Tasks of {@link activeId} only — parents and subtasks together. */
  tasks: DbProjectTask[];
  loading: boolean;

  /** Passing `opts` also sets {@link query}; omitting it re-runs the current one. */
  loadProjects: (opts?: GetProjectsOptions) => Promise<void>;
  open: (id: number | null) => Promise<void>;
  /** Re-read the list and the open project's tasks. */
  reload: () => Promise<void>;

  createProject: (input: CreateProjectInput) => Promise<number>;
  updateProject: (id: number, patch: UpdateProjectInput) => Promise<void>;
  archiveProject: (id: number) => Promise<void>;
  unarchiveProject: (id: number) => Promise<void>;
  deleteProject: (id: number) => Promise<void>;

  createTask: (input: CreateTaskInput) => Promise<number>;
  updateTask: (id: number, patch: UpdateTaskInput) => Promise<void>;
  deleteTask: (id: number) => Promise<void>;
  moveTask: (id: number, columnId: string, beforeId: number | null, afterId: number | null) => Promise<void>;
  /** `null` files the task under no project. */
  refileTask: (id: number, projectId: number | null) => Promise<void>;
}

export const useProjectsStore = create<ProjectsState>((set, get) => ({
  projects: [],
  counts: new Map(),
  query: {},
  activeId: null,
  tasks: [],
  loading: false,

  loadProjects: async (opts) => {
    const query = opts ?? get().query;
    set({ query });
    set(await readList(query));
  },

  open: async (id) => {
    if (id == null) {
      set({ activeId: null, tasks: [] });
      return;
    }
    if (get().activeId === id) return;
    // The old board's tasks stay up until the query lands (no empty flash);
    // the answer is dropped if the user switched again meanwhile.
    set({ activeId: id, loading: true });
    const tasks = await getTasks(id).catch(() => [] as DbProjectTask[]);
    if (get().activeId === id) set({ tasks, loading: false });
  },

  reload: async () => {
    const id = get().activeId;
    const list = await readList(get().query);
    if (id == null) {
      set(list);
      return;
    }
    const tasks = await getTasks(id).catch(() => [] as DbProjectTask[]);
    if (get().activeId === id) set({ ...list, tasks });
    else set(list);
  },

  // Thin on purpose: only state a re-read cannot fix, i.e. clearing `activeId`.

  createProject,

  updateProject,

  archiveProject: async (id) => {
    await archiveProject(id);
    if (get().activeId === id) {
      const still = await getProject(id);
      if (!still || still.status !== "active") set({ activeId: null, tasks: [] });
    }
  },

  unarchiveProject,

  deleteProject: async (id) => {
    await deleteProject(id);
    if (get().activeId === id) set({ activeId: null, tasks: [] });
  },

  createTask,

  updateTask,

  deleteTask,

  moveTask,

  refileTask,
}));

async function readList(
  query: GetProjectsOptions,
): Promise<{ projects: DbProject[]; counts: Map<number, ProjectTaskCounts> }> {
  const projects = await getProjects(query);
  const counts = await getTaskCounts(projects.map((p) => p.id));
  return { projects, counts };
}
