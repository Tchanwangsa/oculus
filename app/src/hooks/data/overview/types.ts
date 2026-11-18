import type { Lecture } from "@/lib/db";
import type { RecordingStatus, SubjectWeeks } from "@/lib/weeks";

export interface ActivityItem {
  id: string;
  kind: "file" | "page" | "announcement" | "ed" | "recording";
  title: string;
  date: Date;
  href: string;
  isNew: boolean;
  badge: string;
  recordingStatus: RecordingStatus | null;
}

export interface DeadlineItem {
  id: string;
  kind: "due" | "task";
  title: string;
  date: Date;
  href: string | null;
}

export interface EventItem {
  id: string;
  kind: "class" | "recording";
  title: string;
  date: Date;
  end: Date | null;
  allDay: boolean;
  location: string | null;
  href: string | null;
  past: boolean;
}

export interface YoursItem {
  id: string;
  kind: "chat" | "document" | "whiteboard" | "notebook" | "upload";
  title: string;
  date: Date;
  href: string;
}

export interface WeekProgress {
  weekKey: number;
  watched: number;
  total: number;
}

export interface SubjectOverviewData {
  weeks: SubjectWeeks;
  activity: ActivityItem[];
  comingUp: DeadlineItem[];
  events: EventItem[];
  yours: YoursItem[];
  behind: { count: number; lectures: Lecture[] };
  progress: WeekProgress[];
  revisionDue: number;
  sinceLastVisit: number | null;
  markVisited: () => void;
  loading: boolean;
  error: string | null;
}
