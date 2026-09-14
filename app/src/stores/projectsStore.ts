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
} from "@/lib/projects";

/**
 * The project list and the open project's tasks.
 *
 * All reads and writes go through `app/src/lib/projects.ts` — this store holds
 * what is on screen and nothing else. It deliberately does **not** call Tauri
 * `listen()`: `app/src/hooks/useBackendEvents.ts` is the single app-level
 * bridge for backend events, and a store that subscribed on its own would be a
 * second one.
 *
 * **A write does not re-read.** Every write in the lib layer fires
 * `PROJECTS_UPDATED_EVENT` on `window`, and that is the only refresh path:
 * a page showing project data listens for it and calls {@link reload}, the way
 * the calendar listens for `CALENDAR_UPDATED_EVENT`. Refreshing here *as well*
 * meant a single drag cost two full task reads plus a project-list read, and
 * the two paths could land out of order. Having only the event left has a
 * second virtue: a write made in the UI and a write made by the chat agent
 * through the CLI arrive by the same door, so nothing can work for one and not
 * the other. What the wrappers below still do is the bookkeeping a re-read
 * cannot express — clearing {@link activeId} when the open project stops being
 * something to look at.
 */
interface ProjectsState {
  projects: DbProject[];
  /**
   * What {@link projects} is filtered by, as state rather than a hidden
   * module-level latch.
   *
   * A component can read it to say what it is showing, and — the reason it
   * moved out of a closure — `loadProjects()` with no argument re-runs *this*
   * query rather than whatever the last caller happened to ask for. Going from
   * a subject's projects to the index is `loadProjects({})`, and forgetting to
   * pass anything can no longer leave the index quietly filtered to a subject.
   */
  query: GetProjectsOptions;
  /**
   * Finished/total per project in {@link projects}, keyed by id.
   *
   * Loaded with the list and from the same query, rather than by each row
   * asking: `getTaskCounts` is one grouped read for every id at once, and a
   * row that fetched its own would fan out a query per project on every
   * refresh. Every id in {@link projects} has an entry — 0/0 for a project
   * with no tasks — so a missing key is a bug, not an empty project.
   */
  counts: Map<number, ProjectTaskCounts>;
  /** The project whose board is on screen; `null` is the list. */
  activeId: number | null;
  /** Tasks of {@link activeId} only — parents and subtasks together. */
  tasks: DbProjectTask[];
  loading: boolean;

  /** Read the list. Passing `opts` also sets {@link query}; omitting it
   *  re-runs the current one. */
  loadProjects: (opts?: GetProjectsOptions) => Promise<void>;
  open: (id: number | null) => Promise<void>;
  /** Re-read the open project and its tasks — what a write path refreshes on,
   *  and what a `PROJECTS_UPDATED_EVENT` listener can call. */
  reload: () => Promise<void>;

  createProject: (input: CreateProjectInput) => Promise<number>;
  updateProject: (id: number, patch: UpdateProjectInput) => Promise<void>;
  archiveProject: (id: number) => Promise<void>;
  deleteProject: (id: number) => Promise<void>;

  createTask: (input: CreateTaskInput) => Promise<number>;
  updateTask: (id: number, patch: UpdateTaskInput) => Promise<void>;
  deleteTask: (id: number) => Promise<void>;
  moveTask: (id: number, columnId: string, beforeId: number | null, afterId: number | null) => Promise<void>;
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
    // The tasks of the board being left stay up for the beat the query takes —
    // clearing here would flash an empty board on every switch. Both answers
    // are guarded against a switch while the query was in flight, the way
    // `harnessStore.open` is: a fast click through three projects must not
    // land the first one's rows under the third one's name.
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

  // ── Write wrappers ────────────────────────────────────────────────────────
  // Thin on purpose: the lib call, and then only the state the event cannot
  // put right by re-reading. The refresh itself is PROJECTS_UPDATED_EVENT's —
  // see the note at the top of this file.

  createProject: async (input) => createProject(input),

  updateProject: async (id, patch) => updateProject(id, patch),

  archiveProject: async (id) => {
    await archiveProject(id);
    // An archived project drops out of the default list, so a board still
    // showing it would be pointing at a row the list no longer has.
    if (get().activeId === id) {
      const still = await getProject(id);
      if (!still || still.status !== "active") set({ activeId: null, tasks: [] });
    }
  },

  deleteProject: async (id) => {
    await deleteProject(id);
    if (get().activeId === id) set({ activeId: null, tasks: [] });
  },

  createTask: async (input) => createTask(input),

  updateTask: async (id, patch) => updateTask(id, patch),

  deleteTask: async (id) => deleteTask(id),

  moveTask: async (id, columnId, beforeId, afterId) =>
    moveTask(id, columnId, beforeId, afterId),
}));

/** The list and its counts, which are always read together — one query for the
 *  rows and one grouped query for every row's tally. */
async function readList(
  query: GetProjectsOptions,
): Promise<{ projects: DbProject[]; counts: Map<number, ProjectTaskCounts> }> {
  const projects = await getProjects(query);
  const counts = await getTaskCounts(projects.map((p) => p.id));
  return { projects, counts };
}
