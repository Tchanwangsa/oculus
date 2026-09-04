/**
 * Starter automations.
 *
 * An empty canvas is a fair question — "what is this for?" — and these are the
 * answer. Each template is a complete graph: real nodes, real config, real
 * positions, wired left to right so it reads the moment it opens.
 *
 * Two rules hold the whole file together, and both come from how the canvas
 * treats a graph it did not draw:
 *
 * 1. **Every port id here must exist.** `AutomationCanvas` prunes any wire
 *    whose port has gone, so a typo does not fail loudly — it silently opens a
 *    graph with a missing wire, which is worse than shipping no template.
 * 2. **No wire lands on a `files` slot from an `any` port.** `canConnect`
 *    refuses that pairing, so a condition's branch cannot feed "Summarise each
 *    file": the user could never redraw it after deleting it. Conditions
 *    therefore sit *after* the summarise node, never before — which costs
 *    nothing, because a `summaries` value is a promise of work rather than the
 *    work, and a branch nobody reads never pays for a model call.
 *
 * Templates land disabled (see `AutomationsPage`): most of them spend tokens,
 * and several want a subject or a model chosen first.
 */

import type {
  AutomationGraph,
  AutomationLink,
  AutomationNode,
  NodeKind,
} from "@/lib/automations";

export type TemplateGroup = "start" | "sync" | "deadlines" | "weekly" | "reading";

export interface AutomationTemplate {
  id: string;
  name: string;
  /** One line, second person, about what it does *for you* — not what nodes
   *  it contains. The graph is right there to be read. */
  description: string;
  group: TemplateGroup;
  graph: AutomationGraph;
}

/** Picker order. "Start here" first, because the first template someone opens
 *  should be the one that explains the feature. */
export const TEMPLATE_GROUPS: { id: TemplateGroup; label: string }[] = [
  { id: "start", label: "Start here" },
  { id: "sync", label: "Around a sync" },
  { id: "deadlines", label: "Deadlines" },
  { id: "weekly", label: "Weekly rhythm" },
  { id: "reading", label: "Reading and triage" },
];

// ── Builders ─────────────────────────────────────────────────────────────────
//
// Positions are laid out on the canvas' own grid — 300 across a column, ~140
// between parallel rows — so a template opens looking like the auto-layout the
// editor would have produced, only better.

const COL = (i: number) => 80 + i * 300;
const ROW = (i: number) => 80 + i * 140;

const n = (
  id: string,
  kind: NodeKind,
  col: number,
  row: number,
  config: Record<string, any>,
): AutomationNode => ({ id, kind, config, position: { x: COL(col), y: ROW(row) } });

const w = (from: string, fromPort: string, to: string, toPort: string): AutomationLink => ({
  from,
  fromPort,
  to,
  toPort,
});

/** A rule's operands, spelled out so the templates below read as sentences. */
const inputCount = { kind: "input", field: "count" } as const;
const inputText = { kind: "input", field: "text" } as const;
const num = (v: number) => ({ kind: "number", n: v });
const text = (v: string) => ({ kind: "text", text: v });
/** Unary ops ignore `right`, but the shape wants one. */
const NOTHING = text("");

const daily = (timeOfDay: string) => ({
  scheduleKind: "daily",
  timeOfDay,
  intervalMinutes: 360,
  days: [1, 2, 3, 4, 5],
});

const weekly = (days: number[], timeOfDay: string) => ({
  scheduleKind: "weekly",
  timeOfDay,
  intervalMinutes: 360,
  days,
});

const everyMinutes = (intervalMinutes: number) => ({
  scheduleKind: "interval",
  timeOfDay: "09:00",
  intervalMinutes,
  days: [1, 2, 3, 4, 5],
});

/** The instruction most summarise nodes want: enough to decide whether to open
 *  the file, and never more than a paragraph per file. */
const SUMMARISE_DEFAULT =
  "In three or four sentences: what this document covers, and anything it asks me to do or hand in. No preamble.";

const TERSE_SYSTEM =
  "You are the study assistant for a University of Melbourne student. Be specific and brief. Use only the material you are given, never invent course content, and skip preambles and sign-offs.";

// ── Templates ────────────────────────────────────────────────────────────────

export const AUTOMATION_TEMPLATES: AutomationTemplate[] = [
  {
    id: "morning-briefing",
    name: "Morning briefing",
    description:
      "At 07:30 it reads your next two days and your unread Inbox, and writes you one page: what's on, what's due, what to prep.",
    group: "start",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1.4, daily("07:30")),
        n("calendar", "source.calendar", 1, 0.5, {
          kinds: ["class", "due"],
          days: 2,
          subjectIds: [],
        }),
        n("inbox", "source.inbox", 1, 2.3, { scope: "unread", days: 3, limit: 15 }),
        n("brief", "action.ai", 2, 1.4, {
          slots: [
            { id: "schedule", label: "Next two days" },
            { id: "unread", label: "Unread Inbox" },
          ],
          system: TERSE_SYSTEM,
          prompt: [
            "Good morning. It is {{date}}.",
            "",
            "Classes and deadlines in the next two days:",
            "{{schedule}}",
            "",
            "Unread in my Inbox ({{unread.count}} items):",
            "{{unread}}",
            "",
            "Write my briefing in three short sections:",
            "",
            "**Today** — what is on and when, in time order, with anything I need to have read beforehand.",
            "**Due soon** — anything due in the next two days, most urgent first, with the date.",
            "**Prep** — the two or three things worth doing today, each with one line on why.",
            "",
            'If a section has nothing in it, write "Nothing" and move on rather than padding it out.',
          ].join("\n"),
        }),
        n("deliver", "action.inbox", 3, 1.4, {
          title: "Morning briefing — {{date}}",
          body: "{{input}}",
        }),
      ],
      links: [
        w("clock", "then", "calendar", "then"),
        w("clock", "then", "inbox", "then"),
        w("calendar", "events", "brief", "schedule"),
        w("inbox", "items", "brief", "unread"),
        w("brief", "text", "deliver", "input"),
      ],
    },
  },

  {
    id: "sync-digest",
    name: "Sync digest",
    description:
      "Every time a sync finishes, each new or changed file gets a short summary and they all land in your Inbox as one item.",
    group: "start",
    graph: {
      nodes: [
        n("synced", "trigger.event", 0, 1, { event: "sync-complete" }),
        n("summarise", "action.summarise", 1, 1, { instruction: SUMMARISE_DEFAULT }),
        n("deliver", "action.inbox", 2, 1, {
          title: "Sync digest — {{input.count}} files",
          body: "{{input}}",
        }),
      ],
      links: [
        w("synced", "changed", "summarise", "files"),
        w("summarise", "summaries", "deliver", "input"),
      ],
    },
  },

  {
    id: "sync-on-launch",
    name: "Sync when Oculus opens",
    description:
      "Pulls Canvas, Ed and Echo360 the moment you open the app, so what you read is what your subjects actually say.",
    group: "start",
    graph: {
      nodes: [
        n("launched", "trigger.event", 0, 1, { event: "app-start" }),
        n("sync", "action.sync", 1, 1, {}),
      ],
      links: [w("launched", "then", "sync", "then")],
    },
  },

  {
    id: "keep-me-synced",
    name: "Keep me synced",
    description:
      "Syncs every six hours and writes you a digest — but only when something actually changed. A quiet sync lands nothing.",
    group: "sync",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1, everyMinutes(360)),
        n("sync", "action.sync", 1, 1, {}),
        n("summarise", "action.summarise", 2, 1, { instruction: SUMMARISE_DEFAULT }),
        n("gate", "condition.if", 3, 1, {
          branches: [
            {
              id: "changed",
              label: "Something changed",
              match: "all",
              rules: [{ left: inputCount, op: "gt", right: num(0) }],
            },
          ],
          otherwise: false,
        }),
        n("deliver", "action.inbox", 4, 1, {
          title: "Sync digest — {{input.count}} files",
          body: "{{input}}",
        }),
      ],
      links: [
        w("clock", "then", "sync", "then"),
        w("sync", "changed", "summarise", "files"),
        w("summarise", "summaries", "gate", "input"),
        w("gate", "changed", "deliver", "input"),
      ],
    },
  },

  {
    id: "prep-pack",
    name: "New lecture, prep pack",
    description:
      "After a sync, today's new material becomes five revision questions with answers you can cover up.",
    group: "sync",
    graph: {
      nodes: [
        n("synced", "trigger.event", 0, 1, { event: "sync-complete" }),
        n("today", "source.files", 1, 1, {
          days: 1,
          subjectIds: [],
          category: null,
          limit: 20,
        }),
        n("summarise", "action.summarise", 2, 1, {
          instruction:
            "Summarise this in five or six bullets: the ideas it introduces, the definitions and results worth remembering, and anything it asks me to do.",
        }),
        n("gate", "condition.if", 3, 1, {
          branches: [
            {
              id: "arrived",
              label: "Something arrived",
              match: "all",
              rules: [{ left: inputCount, op: "gt", right: num(0) }],
            },
          ],
          otherwise: false,
        }),
        n("questions", "action.ai", 4, 1, {
          slots: [{ id: "material", label: "Today's material" }],
          system: TERSE_SYSTEM,
          prompt: [
            "Here are notes on the course material that arrived today:",
            "",
            "{{material}}",
            "",
            "Turn it into a revision pack:",
            "",
            "**Questions** — five numbered questions that test whether I actually understood this. Mix recall and application.",
            "**Answers** — below the questions, a two or three line answer to each, so I can cover them up.",
            "**Gaps** — anything the material assumes I already know but never explains.",
            "",
            "Base every question on the material above. Do not invent content that is not there.",
          ].join("\n"),
        }),
        n("deliver", "action.inbox", 5, 1, {
          title: "Prep pack — {{date}}",
          body: "{{input}}",
        }),
      ],
      links: [
        w("synced", "changed", "today", "then"),
        w("today", "files", "summarise", "files"),
        w("summarise", "summaries", "gate", "input"),
        w("gate", "arrived", "questions", "material"),
        w("questions", "text", "deliver", "input"),
      ],
    },
  },

  {
    id: "announcement-watch",
    name: "Announcement watch",
    description:
      "Watches for new Canvas announcements and tells you what each one wants from you, on screen and in your Inbox.",
    group: "sync",
    graph: {
      nodes: [
        n("synced", "trigger.event", 0, 1.5, { event: "sync-complete" }),
        n("posts", "source.files", 1, 1.5, {
          days: 2,
          subjectIds: [],
          category: "announcement",
          limit: 10,
        }),
        n("summarise", "action.summarise", 2, 1.5, {
          instruction:
            "In two sentences: what this announcement says, and what it asks me to do, if anything. Quote any date it gives.",
        }),
        n("gate", "condition.if", 3, 1.5, {
          branches: [
            {
              id: "posted",
              label: "New announcements",
              match: "all",
              rules: [{ left: inputCount, op: "gt", right: num(0) }],
            },
          ],
          otherwise: false,
        }),
        n("deliver", "action.inbox", 4, 0.8, {
          title: "Announcements — {{date}}",
          body: "{{input}}",
        }),
        n("ping", "action.notify", 4, 2.3, {
          title: "New announcement",
          body: "{{input.count}} posted — the details are in your Inbox.",
        }),
      ],
      links: [
        w("synced", "changed", "posts", "then"),
        w("posts", "files", "summarise", "files"),
        w("summarise", "summaries", "gate", "input"),
        w("gate", "posted", "deliver", "input"),
        w("gate", "posted", "ping", "input"),
      ],
    },
  },

  {
    id: "big-sync-small-sync",
    name: "Big drop, small drop",
    description:
      "Sizes the sync before it reports: ten files or more get an AI overview of what to read first, a handful just get their summaries.",
    group: "sync",
    graph: {
      nodes: [
        n("synced", "trigger.event", 0, 1.4, { event: "sync-complete" }),
        n("summarise", "action.summarise", 1, 1.4, { instruction: SUMMARISE_DEFAULT }),
        // First match wins, so the "ten or more" branch has to come first —
        // otherwise every big drop would also satisfy "a handful" and take
        // that path instead.
        n("size", "condition.if", 2, 1.4, {
          branches: [
            {
              id: "big",
              label: "Ten or more",
              match: "all",
              rules: [{ left: inputCount, op: "gte", right: num(10) }],
            },
            {
              id: "few",
              label: "A handful",
              match: "all",
              rules: [{ left: inputCount, op: "gte", right: num(1) }],
            },
          ],
          otherwise: false,
        }),
        n("overview", "action.ai", 3, 0.4, {
          slots: [{ id: "batch", label: "Everything new" }],
          system: TERSE_SYSTEM,
          prompt: [
            "{{batch.count}} files landed in one go. Here is a summary of each:",
            "",
            "{{batch}}",
            "",
            "Write an overview I can read in a minute:",
            "",
            "**The shape of it** — what this drop is, in two lines. A week of lectures? A new assignment? Admin?",
            "**Read first** — the three files that matter most, and one line on why each.",
            "**Can wait** — everything else, grouped by subject, one line per group.",
            "",
            "Do not repeat the summaries back to me.",
          ].join("\n"),
        }),
        n("deliverBig", "action.inbox", 4, 0.4, {
          title: "Big drop — {{date}}",
          body: "{{input}}",
        }),
        n("deliverFew", "action.inbox", 3, 2.4, {
          title: "Sync digest — {{input.count}} files",
          body: "{{input}}",
        }),
      ],
      links: [
        w("synced", "changed", "summarise", "files"),
        w("summarise", "summaries", "size", "input"),
        w("size", "big", "overview", "batch"),
        w("overview", "text", "deliverBig", "input"),
        w("size", "few", "deliverFew", "input"),
      ],
    },
  },

  {
    id: "deadline-radar",
    name: "Deadline radar",
    description:
      "Each evening it sweeps the next fortnight for due dates and triages them by urgency, with one thing to start tonight.",
    group: "deadlines",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1.4, daily("18:00")),
        n("due", "source.calendar", 1, 1.4, {
          kinds: ["due"],
          days: 14,
          subjectIds: [],
        }),
        n("gate", "condition.if", 2, 1.4, {
          branches: [
            {
              id: "pending",
              label: "Something is due",
              match: "all",
              rules: [{ left: inputText, op: "notEmpty", right: NOTHING }],
            },
          ],
          otherwise: false,
        }),
        n("triage", "action.ai", 3, 0.5, {
          slots: [{ id: "deadlines", label: "Due in 14 days" }],
          system: TERSE_SYSTEM,
          prompt: [
            "Today is {{date}}. These are my deadlines in the next fourteen days:",
            "",
            "{{deadlines}}",
            "",
            "Sort them into **This week** and **Next week**. For each one give:",
            "",
            "- the subject and what is due,",
            "- the date, and how many days away it is,",
            "- one line on the next concrete step that moves it forward.",
            "",
            "Finish with a single sentence naming the one thing to start tonight.",
          ].join("\n"),
        }),
        n("deliver", "action.inbox", 4, 0.5, {
          title: "Deadline radar — {{date}}",
          body: "{{input}}",
        }),
        n("ping", "action.notify", 3, 2.3, {
          title: "Deadlines ahead",
          body: "Something is due in the next fortnight — the triage is in your Inbox.",
        }),
      ],
      links: [
        w("clock", "then", "due", "then"),
        w("due", "events", "gate", "input"),
        w("gate", "pending", "triage", "deadlines"),
        w("triage", "text", "deliver", "input"),
        w("gate", "pending", "ping", "input"),
      ],
    },
  },

  {
    id: "exam-countdown",
    name: "Exam countdown",
    description:
      "A morning check on the coming seven days: if anything is due inside the week, you get a notification naming it.",
    group: "deadlines",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1, daily("08:00")),
        // Seven days is the window, not a filter after the fact: a condition
        // reads a value, not the dates inside it, so "inside a week" has to be
        // asked of the calendar rather than tested downstream.
        n("thisWeek", "source.calendar", 1, 1, {
          kinds: ["due"],
          days: 7,
          subjectIds: [],
        }),
        n("gate", "condition.if", 2, 1, {
          branches: [
            {
              id: "close",
              label: "Due inside the week",
              match: "all",
              rules: [{ left: inputText, op: "notEmpty", right: NOTHING }],
            },
          ],
          otherwise: false,
        }),
        n("ping", "action.notify", 3, 1, {
          title: "Due this week",
          body: "{{input}}",
        }),
      ],
      links: [
        w("clock", "then", "thisWeek", "then"),
        w("thisWeek", "events", "gate", "input"),
        w("gate", "close", "ping", "input"),
      ],
    },
  },

  {
    id: "assignment-spotter",
    name: "Assignment spotter",
    description:
      "Reads new assignment pages after a sync, pulls the due date out of the brief, and puts it in your calendar.",
    group: "deadlines",
    graph: {
      nodes: [
        n("synced", "trigger.event", 0, 1.4, { event: "sync-complete" }),
        n("briefs", "source.files", 1, 1.4, {
          days: 1,
          subjectIds: [],
          category: "assignment",
          limit: 10,
        }),
        n("read", "action.summarise", 2, 1.4, {
          instruction:
            "If this document sets a piece of assessment, state its title, its due date exactly as written, and its weighting. If it does not, reply with just NONE.",
        }),
        n("anyBriefs", "condition.if", 3, 1.4, {
          branches: [
            {
              id: "found",
              label: "Assignment pages",
              match: "all",
              rules: [{ left: inputCount, op: "gt", right: num(0) }],
            },
          ],
          otherwise: false,
        }),
        // One reply, one value downstream — so the model is asked for the date
        // alone. The calendar node templates its title and its date from the
        // same slot, and a date it cannot parse is not an event.
        n("extract", "action.ai", 4, 1.4, {
          slots: [{ id: "briefs", label: "Assignment briefs" }],
          system:
            "Answer with the requested token and nothing else. Never explain, never apologise.",
          prompt: [
            "Today is {{date}}. These notes come from assignment pages Canvas published or changed today:",
            "",
            "{{briefs}}",
            "",
            "If they describe a piece of assessment with a due date, reply with that date on its own line as YYYY-MM-DD and nothing else.",
            "If there is no due date, or none of this is assessment, reply with exactly NONE.",
          ].join("\n"),
        }),
        n("hasDate", "condition.if", 5, 1.4, {
          branches: [
            {
              id: "dated",
              label: "Got a date",
              match: "all",
              // Anchored: "Add to calendar" throws on anything it cannot parse
              // as a date, so the gate has to be as strict as the parser — a
              // reply that merely *contains* a date must not reach it.
              rules: [
                {
                  left: inputText,
                  op: "matches",
                  right: text("^\\s*\\d{4}-\\d{2}-\\d{2}\\s*$"),
                },
              ],
            },
          ],
          otherwise: false,
        }),
        n("book", "action.calendar", 6, 0.5, {
          title: "Assessment due",
          kind: "due",
          when: "template",
          at: "{{input}}",
          offsetDays: 7,
          timeOfDay: "23:59",
          subjectId: null,
          notes:
            "Spotted by Oculus in assignment material published on {{date}}. Open the subject's Assignments tab for the brief.",
        }),
        n("ping", "action.notify", 6, 2.3, {
          title: "Due date spotted",
          body: "{{input}} — added to your calendar. Check the brief on Canvas.",
        }),
      ],
      links: [
        w("synced", "changed", "briefs", "then"),
        w("briefs", "files", "read", "files"),
        w("read", "summaries", "anyBriefs", "input"),
        w("anyBriefs", "found", "extract", "briefs"),
        w("extract", "text", "hasDate", "input"),
        w("hasDate", "dated", "book", "input"),
        w("hasDate", "dated", "ping", "input"),
      ],
    },
  },

  {
    id: "weekly-review",
    name: "Weekly review",
    description:
      "Sunday afternoon: what landed last week, what the coming week holds, and a day-by-day plan that fits inside it.",
    group: "weekly",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1.4, weekly([0], "17:00")),
        n("lastWeek", "source.files", 1, 0.5, {
          days: 7,
          subjectIds: [],
          category: null,
          limit: 40,
        }),
        n("weekAhead", "source.calendar", 1, 2.3, {
          kinds: ["class", "due"],
          days: 7,
          subjectIds: [],
        }),
        n("summarise", "action.summarise", 2, 0.5, {
          instruction:
            "In two or three sentences: what this covers, and anything it asks me to do. No preamble.",
        }),
        n("plan", "action.ai", 3, 1.4, {
          slots: [
            { id: "material", label: "Last week's material" },
            { id: "ahead", label: "The week ahead" },
          ],
          system: TERSE_SYSTEM,
          prompt: [
            "It is {{date}} and I am planning the week.",
            "",
            "Material that arrived in the last seven days ({{material.count}} files):",
            "{{material}}",
            "",
            "Classes and deadlines in the next seven days:",
            "{{ahead}}",
            "",
            "Write my weekly review:",
            "",
            "**What changed** — the themes of the past week per subject, not a file list.",
            "**What's coming** — the week ahead and the deadlines inside it.",
            "**The plan** — a day-by-day plan, two or three items a day, each tied to a subject and a reason. Put work that unblocks a deadline early in the week.",
            "**Catching up** — anything from last week that clearly still needs reading.",
            "",
            "Be realistic about how much fits in a day.",
          ].join("\n"),
        }),
        n("deliver", "action.inbox", 4, 1.4, {
          title: "Weekly review — {{date}}",
          body: "{{input}}",
        }),
      ],
      links: [
        w("clock", "then", "lastWeek", "then"),
        w("clock", "then", "weekAhead", "then"),
        w("lastWeek", "files", "summarise", "files"),
        w("summarise", "summaries", "plan", "material"),
        w("weekAhead", "events", "plan", "ahead"),
        w("plan", "text", "deliver", "input"),
      ],
    },
  },

  {
    id: "quiet-week",
    name: "Quiet week",
    description:
      "Friday afternoon, and only if nothing new arrived all week: a nudge that the silence might be the sync, not the subject.",
    group: "weekly",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1, weekly([5], "16:00")),
        n("week", "source.files", 1, 1, {
          days: 7,
          subjectIds: [],
          category: null,
          limit: 50,
        }),
        n("gate", "condition.if", 2, 1, {
          branches: [
            {
              id: "quiet",
              label: "Nothing new",
              match: "all",
              rules: [{ left: inputCount, op: "eq", right: num(0) }],
            },
          ],
          otherwise: false,
        }),
        n("ping", "action.notify", 3, 1, {
          title: "A quiet week",
          body: "No new course material in the last seven days. Worth checking a subject by hand.",
        }),
      ],
      links: [
        w("clock", "then", "week", "then"),
        w("week", "files", "gate", "input"),
        w("gate", "quiet", "ping", "input"),
      ],
    },
  },

  {
    id: "inbox-triage",
    name: "Inbox triage",
    description:
      "Collapses three days of unread Inbox items into five bullets answering one question: what actually needs you?",
    group: "reading",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1, daily("21:00")),
        n("unread", "source.inbox", 1, 1, { scope: "unread", days: 3, limit: 25 }),
        n("gate", "condition.if", 2, 1, {
          branches: [
            {
              id: "waiting",
              label: "Anything unread",
              match: "all",
              rules: [{ left: inputText, op: "notEmpty", right: NOTHING }],
            },
          ],
          otherwise: false,
        }),
        n("collapse", "action.ai", 3, 1, {
          slots: [{ id: "items", label: "Unread items" }],
          system: TERSE_SYSTEM,
          prompt: [
            "These are the Oculus Inbox items I have not read, from the last three days:",
            "",
            "{{items}}",
            "",
            "Collapse them into at most five bullets that answer one question: what actually needs me?",
            "",
            "- Lead each bullet with the subject code.",
            "- Say what the thing is and what it wants from me, in one sentence.",
            "- Put anything with a deadline first.",
            "- End with one line naming what you left out and why it did not make the cut.",
            "",
            "If none of it needs anything from me, say so in one line rather than finding five things.",
          ].join("\n"),
        }),
        n("deliver", "action.inbox", 4, 1, {
          title: "Tonight's triage",
          body: "{{input}}",
        }),
      ],
      links: [
        w("clock", "then", "unread", "then"),
        w("unread", "items", "gate", "input"),
        w("gate", "waiting", "collapse", "items"),
        w("collapse", "text", "deliver", "input"),
      ],
    },
  },

  {
    id: "ed-catchup",
    name: "Ed Discussion catch-up",
    description:
      "Middle of the day, it reads the threads posted since yesterday and tells you which two are worth opening.",
    group: "reading",
    graph: {
      nodes: [
        n("clock", "trigger.schedule", 0, 1, daily("12:30")),
        n("threads", "source.files", 1, 1, {
          days: 1,
          subjectIds: [],
          category: "ed",
          limit: 20,
        }),
        n("summarise", "action.summarise", 2, 1, {
          instruction:
            "In two sentences: what is being asked or discussed here, and the answer if one was given.",
        }),
        n("gate", "condition.if", 3, 1, {
          branches: [
            {
              id: "posted",
              label: "New threads",
              match: "all",
              rules: [{ left: inputCount, op: "gt", right: num(0) }],
            },
          ],
          otherwise: false,
        }),
        n("catchup", "action.ai", 4, 1, {
          slots: [{ id: "threads", label: "Today's threads" }],
          system: TERSE_SYSTEM,
          prompt: [
            "These are the Ed Discussion threads from the last day:",
            "",
            "{{threads}}",
            "",
            "Give me the catch-up:",
            "",
            "**Answered** — questions that already have an answer, one line each, with the answer itself.",
            "**Open** — questions still hanging, one line each.",
            "**Worth opening** — the two or three threads I should actually read, and why.",
            "",
            "Skip small talk and logistics that do not affect me.",
          ].join("\n"),
        }),
        n("deliver", "action.inbox", 5, 1, {
          title: "Ed catch-up — {{date}}",
          body: "{{input}}",
        }),
      ],
      links: [
        w("clock", "then", "threads", "then"),
        w("threads", "files", "summarise", "files"),
        w("summarise", "summaries", "gate", "input"),
        w("gate", "posted", "catchup", "threads"),
        w("catchup", "text", "deliver", "input"),
      ],
    },
  },
];

/** Templates in picker order, grouped. Groups with nothing in them are
 *  dropped, so adding a template is the only edit adding a template needs. */
export function templatesByGroup(): { label: string; templates: AutomationTemplate[] }[] {
  return TEMPLATE_GROUPS.map((g) => ({
    label: g.label,
    templates: AUTOMATION_TEMPLATES.filter((t) => t.group === g.id),
  })).filter((g) => g.templates.length > 0);
}

/** Unused today, but the ids are the stable handle a template is referred to
 *  by — keep the lookup next to the list so it stays honest. */
export const templateById = (id: string): AutomationTemplate | undefined =>
  AUTOMATION_TEMPLATES.find((t) => t.id === id);
