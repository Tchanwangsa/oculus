import {
  ChatCircle, CheckSquare, FileText, Megaphone, NotePencil, Notebook, PencilLine, Scribble,
  UploadSimple, UsersThree, VideoCamera, type Icon,
} from "@phosphor-icons/react";
import type { ActivityItem, DeadlineItem, EventItem, YoursItem } from "@/hooks/data/overview";
import type { DbFile } from "@/lib/db";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";

export interface Glyph {
  Icon: Icon;
  tone: string;
}

const MUTED = "text-muted-foreground";

/** Library rows take their reference colours, as everywhere else they appear. */
export function activityGlyph(item: ActivityItem, file: DbFile | undefined): Glyph {
  if (item.kind === "recording") return { Icon: VideoCamera, tone: "text-reference-lecture" };
  if (item.kind === "announcement") return { Icon: Megaphone, tone: MUTED };
  if (!file) return { Icon: FileText, tone: MUTED };
  const { Icon, tone } = fileReferenceAppearance(file);
  return { Icon, tone };
}

export function deadlineGlyph(item: DeadlineItem): Glyph {
  return item.kind === "task"
    ? { Icon: CheckSquare, tone: "text-reference-task" }
    : { Icon: PencilLine, tone: MUTED };
}

export function eventGlyph(item: EventItem): Glyph {
  return item.kind === "recording"
    ? { Icon: VideoCamera, tone: "text-reference-lecture" }
    : { Icon: UsersThree, tone: MUTED };
}

const YOURS: Record<YoursItem["kind"], Glyph> = {
  chat: { Icon: ChatCircle, tone: MUTED },
  document: { Icon: NotePencil, tone: "text-reference-page" },
  canvas: { Icon: Scribble, tone: "text-reference-page" },
  notebook: { Icon: Notebook, tone: "text-reference-page" },
  upload: { Icon: UploadSimple, tone: MUTED },
};

export function yoursGlyph(item: YoursItem): Glyph {
  return YOURS[item.kind];
}
