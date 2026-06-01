import { Bell, Clock, ClipboardList, Megaphone, AlertCircle, CheckCircle2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

type NotifType = "deadline" | "announcement" | "grade" | "alert";

interface Notification {
  id: number;
  type: NotifType;
  course: string;
  title: string;
  body: string;
  time: string;
  dueIn?: string;
  read: boolean;
}

const NOTIFICATIONS: Notification[] = [
  {
    id: 1,
    type: "deadline",
    course: "COMP30023",
    title: "Assignment 2 due",
    body: "Computer Networks Assignment 2 — Socket Programming is due soon.",
    time: "2 hours ago",
    dueIn: "3 days",
    read: false,
  },
  {
    id: 2,
    type: "announcement",
    course: "SWEN30006",
    title: "Project 2 brief released",
    body: "The Project 2 specification is now available on Canvas. Please review before next lecture.",
    time: "5 hours ago",
    read: false,
  },
  {
    id: 3,
    type: "deadline",
    course: "COMP30027",
    title: "Workshop 4 submission",
    body: "Workshop 4 Jupyter notebook submission deadline approaching.",
    time: "1 day ago",
    dueIn: "6 days",
    read: false,
  },
  {
    id: 4,
    type: "announcement",
    course: "COMP30023",
    title: "Lecture recording available",
    body: "Week 5 lecture recording is now available on Echo360.",
    time: "2 days ago",
    read: true,
  },
  {
    id: 5,
    type: "alert",
    course: "SWEN30006",
    title: "Consultation times updated",
    body: "Week 6 consultation times have changed. Check the updated schedule.",
    time: "3 days ago",
    read: true,
  },
];

const TYPE_CONFIG: Record<NotifType, { icon: typeof Bell; color: string; badgeVariant: "default" | "warning" | "accent" | "destructive" | "success" | "secondary" | "outline" }> = {
  deadline:     { icon: Clock,         color: "text-warning",     badgeVariant: "warning" },
  announcement: { icon: Megaphone,     color: "text-accent",      badgeVariant: "accent" },
  grade:        { icon: CheckCircle2,  color: "text-success",     badgeVariant: "success" },
  alert:        { icon: AlertCircle,   color: "text-destructive", badgeVariant: "destructive" },
};

export default function NotificationsPage() {
  const unread = NOTIFICATIONS.filter((n) => !n.read).length;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 px-6 h-14 border-b border-border shrink-0">
        <Bell size={16} className="text-primary" />
        <span className="font-semibold text-foreground">Notifications</span>
        {unread > 0 && <Badge variant="accent">{unread} unread</Badge>}
        <button className="ml-auto text-xs text-muted-foreground hover:text-foreground transition-colors">
          Mark all read
        </button>
      </div>

      {/* Filter row */}
      <div className="flex items-center gap-2 px-6 py-3 border-b border-border shrink-0">
        {(["All", "Deadlines", "Announcements", "Alerts"] as const).map((f) => (
          <button
            key={f}
            className={cn(
              "px-3 py-1.5 rounded-full text-xs font-medium transition-colors",
              f === "All"
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:text-foreground hover:bg-surface"
            )}
          >
            {f}
          </button>
        ))}
      </div>

      {/* Notification list */}
      <div className="flex-1 overflow-y-auto px-6 py-4 space-y-2">
        {NOTIFICATIONS.map((notif) => {
          const config = TYPE_CONFIG[notif.type];
          const Icon = config.icon;

          return (
            <div
              key={notif.id}
              className={cn(
                "flex gap-4 p-4 rounded-xl border transition-colors cursor-pointer",
                notif.read
                  ? "border-border bg-card hover:bg-surface"
                  : "border-primary/20 bg-primary/5 hover:bg-primary/8"
              )}
            >
              <div
                className={cn(
                  "w-9 h-9 rounded-xl flex items-center justify-center shrink-0 mt-0.5",
                  notif.read ? "bg-surface-raised" : "bg-primary/10"
                )}
              >
                <Icon size={16} className={notif.read ? "text-muted-foreground" : config.color} />
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-0.5 flex-wrap">
                  <span className="text-xs font-semibold text-foreground">{notif.title}</span>
                  <Badge variant="secondary">{notif.course}</Badge>
                  {notif.dueIn && (
                    <Badge variant={config.badgeVariant}>Due in {notif.dueIn}</Badge>
                  )}
                  {!notif.read && (
                    <span className="ml-auto w-2 h-2 rounded-full bg-primary shrink-0" />
                  )}
                </div>
                <p className="text-xs text-muted-foreground leading-relaxed">{notif.body}</p>
                <p className="text-[11px] text-muted-foreground/70 mt-1.5">{notif.time}</p>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
