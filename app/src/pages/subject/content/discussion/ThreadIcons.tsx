import { ChatCircle, CheckCircle, Circle, CircleDashed, Megaphone, Question } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** One glyph per thread type, the same in a row and in the Type filter. */
export function ThreadKindIcon({ kind, size = 14 }: { kind: string | null; size?: number }) {
  switch (kind) {
    case "question":
      return <Question size={size} className="shrink-0 text-reference-discussion" />;
    case "post":
      return <ChatCircle size={size} className="shrink-0 text-reference-discussion" />;
    case "announcement":
      return <Megaphone size={size} className="shrink-0 text-muted-foreground" />;
    default:
      return <ChatCircle size={size} className="shrink-0 text-muted-foreground" />;
  }
}

/** A question's answer status (`threadStatus`), the same in a row and in the
 *  Status filter. */
export function ThreadStatusIcon({ status, size = 14 }: { status: string | null; size?: number }) {
  if (status === "resolved") return <CheckCircle size={size} weight="fill" className="shrink-0 text-success" />;
  if (status === "unresolved") return <Circle size={size} className="shrink-0 text-warning" />;
  if (status === "unknown") return <CircleDashed size={size} className="shrink-0 text-muted-foreground" />;
  return null;
}

/** Ed's colour square for a category, as Ed's own sidebar draws it; a muted
 *  one when the colour is unknown. */
export function CategoryDot({ color, className }: { color: string | null; className?: string }) {
  return (
    <span
      aria-hidden
      className={cn("size-2 shrink-0 rounded-[2px]", !color && "bg-muted-foreground/40", className)}
      style={color ? { backgroundColor: color } : undefined}
    />
  );
}
