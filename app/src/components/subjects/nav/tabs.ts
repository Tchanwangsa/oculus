import { tasksHref } from "@/components/projects/tasks/taskViews";
import {
  ChalkboardTeacher,
  ChatsCircle,
  Folder,
  House,
  ListChecks,
  Megaphone,
  PencilLine,
  Stack,
  VideoCamera,
} from "@phosphor-icons/react";

export const TABS = [
  { to: ".",             label: "Overview",      icon: House,          end: true },
  { to: "modules",       label: "Modules",       icon: Stack,          end: false },
  { to: "lectures",      label: "Lectures",      icon: VideoCamera,    end: false },
  { to: "files",         label: "Files",         icon: Folder,         end: false },
  { to: "announcements", label: "Announcements", icon: Megaphone,      end: false },
  { to: "assignments",   label: "Assignments",   icon: PencilLine,     end: false },
  { to: "discussion",    label: "Discussion",    icon: ChatsCircle,    end: false },
  { to: "lessons",       label: "Lessons",       icon: ChalkboardTeacher, end: false },
  { to: "tasks",         label: "Tasks",         icon: ListChecks,         end: false },
] as const;

export const TAB_PATHS = new Set<string>(TABS.map((tab) => tab.to));

/** Breadcrumb categories use the same glyph as their subject navigation row. */
export function subjectTabIcon(path: string) {
  return TABS.find(tab => tab.to === path.split("/")[0])?.icon ?? House;
}

/** Tasks lives in the planning section; the rest stay within the subject. */
export function subjectTabHref(tab: string, subjectId: number): string {
  return tab === "tasks" ? tasksHref("all", null, subjectId) : tab;
}
