import {
  ArrowsClockwise,
  BellRinging,
  CalendarBlank,
  CalendarDots,
  CalendarPlus,
  Files,
  GitBranch,
  Lightning,
  Sparkle,
  Tray,
  TrayArrowUp,
  type Icon,
} from "@phosphor-icons/react";
import type { NodeKind } from "@/lib/automations";

/**
 * What the editor can put on the canvas: one entry per node kind, holding the
 * palette copy and the config a freshly dropped node starts with.
 *
 * Defaults matter — a node dropped here is live the moment it is connected, so
 * every kind has to start in a state that either runs sensibly or reads as
 * obviously unfinished in the inspector.
 *
 * Ports are not here: they are derived from a node's kind and config by
 * `inputPorts` / `outputPorts` in `lib/automations.ts`, because an event
 * trigger's outputs change with the event it listens for.
 */
export interface NodeSpec {
  kind: NodeKind;
  title: string;
  blurb: string;
  icon: Icon;
  group: "trigger" | "source" | "logic" | "action";
  /** Extra words the palette search matches on top of title and blurb, so a
   *  node can be found by what it is *for* rather than by its name — nobody
   *  looking for a deadline reminder searches "schedule". */
  keywords?: string[];
  defaults: Record<string, any>;
}

export const NODE_SPECS: NodeSpec[] = [
  {
    kind: "trigger.schedule",
    title: "On a schedule",
    blurb: "Daily, weekly, or every few hours.",
    icon: CalendarBlank,
    group: "trigger",
    keywords: ["cron", "timer", "every", "daily", "weekly", "interval", "clock", "repeat", "morning"],
    defaults: { scheduleKind: "daily", timeOfDay: "09:00", intervalMinutes: 360, days: [1, 2, 3, 4, 5] },
  },
  {
    kind: "trigger.event",
    title: "On an event",
    blurb: "When a sync finishes, or Oculus starts.",
    icon: Lightning,
    group: "trigger",
    keywords: ["sync", "finished", "after", "launch", "startup", "open", "when"],
    defaults: { event: "sync-complete" },
  },
  {
    kind: "source.inbox",
    title: "Read my Inbox",
    blurb: "Recent Inbox items, as one block of text.",
    icon: TrayArrowUp,
    group: "source",
    keywords: ["inbox", "unread", "digest", "items", "notes", "catch up", "recap"],
    defaults: { scope: "unread", days: 7, limit: 20 },
  },
  {
    kind: "source.calendar",
    title: "Read my calendar",
    blurb: "What is coming up: classes, deadlines, lectures.",
    icon: CalendarDots,
    group: "source",
    keywords: ["calendar", "due", "deadline", "assignment", "class", "timetable", "lecture", "upcoming", "week"],
    defaults: { kinds: ["due"], days: 7, subjectIds: [] },
  },
  {
    kind: "source.files",
    title: "Read my files",
    blurb: "Files from your library, by subject and recency.",
    icon: Files,
    group: "source",
    keywords: ["files", "documents", "library", "recent", "pdf", "slides", "readings", "notes"],
    defaults: { days: 7, subjectIds: [], category: null, limit: 50 },
  },
  {
    kind: "condition.if",
    title: "Split the path",
    blurb: "First branch whose rules match takes the value.",
    icon: GitBranch,
    group: "logic",
    keywords: ["if", "only", "branch", "switch", "route", "filter", "rule", "else", "when", "otherwise"],
    defaults: {
      branches: [{ id: "b0", label: "Yes", match: "all", rules: [{ left: { kind: "input", field: "count" }, op: "gt", right: { kind: "number", n: 0 } }] }],
      otherwise: true,
    },
  },
  {
    kind: "action.sync",
    title: "Sync my subjects",
    blurb: "Scrape Canvas, Ed and Echo360.",
    icon: ArrowsClockwise,
    group: "action",
    keywords: ["scrape", "canvas", "ed", "echo360", "fetch", "download", "refresh", "update", "check"],
    defaults: {},
  },
  {
    kind: "action.summarise",
    title: "Summarise each file",
    blurb: "Read the files wired in, one summary each.",
    icon: Sparkle,
    group: "action",
    keywords: ["summarise", "summarize", "summary", "tldr", "digest", "per file", "ai", "read"],
    defaults: { instruction: "" },
  },
  {
    kind: "action.ai",
    title: "Ask AI",
    blurb: "One prompt over the inputs you wire in.",
    icon: Sparkle,
    group: "action",
    keywords: ["ai", "llm", "prompt", "generate", "write", "ask", "model", "chat", "plan"],
    defaults: {
      prompt: "Here is what changed in my course material today:\n\n{{input}}\n\nWrite me a short study plan for tonight.",
      system: "",
      slots: [{ id: "input", label: "Input" }],
    },
  },
  {
    kind: "action.inbox",
    title: "Add to Inbox",
    blurb: "Deliver whatever is wired in.",
    icon: Tray,
    group: "action",
    keywords: ["inbox", "note", "deliver", "save", "item", "digest", "read later"],
    defaults: { title: "{{name}}", body: "{{input}}" },
  },
  {
    kind: "action.calendar",
    title: "Add to calendar",
    blurb: "Write an event or a deadline onto the grid.",
    icon: CalendarPlus,
    group: "action",
    keywords: ["calendar", "event", "deadline", "due", "reminder", "date", "block", "plan"],
    defaults: {
      title: "{{name}}",
      kind: "note",
      when: "offset",
      at: "",
      offsetDays: 1,
      timeOfDay: "09:00",
      subjectId: null,
      notes: "{{input}}",
    },
  },
  {
    kind: "action.notify",
    title: "Send a notification",
    blurb: "A desktop notification.",
    icon: BellRinging,
    group: "action",
    keywords: ["notification", "alert", "notify", "desktop", "ping", "remind", "banner"],
    defaults: { title: "{{name}}", body: "{{input.count}} new or updated files" },
  },
];

export const SPEC_BY_KIND: Record<NodeKind, NodeSpec> = Object.fromEntries(
  NODE_SPECS.map((s) => [s.kind, s]),
) as Record<NodeKind, NodeSpec>;

export const GROUP_LABELS: Record<NodeSpec["group"], string> = {
  trigger: "Triggers",
  source: "Read",
  logic: "Logic",
  action: "Actions",
};

/** Palette order: what starts a run, then what it can look up, then how it
 *  branches, then what it does. */
export const GROUP_ORDER: NodeSpec["group"][] = ["trigger", "source", "logic", "action"];
