import { BookmarkSimple, FilmSlate } from "@phosphor-icons/react";
import { fileIconFor } from "@/lib/files/fileTypes";
import { fileTitle, folderTitle, pathFile } from "@/lib/files/openFile";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { displayCode, humanizeSlug } from "@/lib/format/format";
import { formatPages, formatTimes, type InfoEntry } from "@/lib/harness/threadInfo";
import { parsedSourceOf } from "@/lib/citations/parse";
import { InfoRow } from "./InfoRow";
import { openTarget } from "./openTarget";

const ICON = 15;
const base = (p: string) => p.replace(/\/+$/, "").split("/").pop() ?? p;

/** A memory's row names it as its index does, by the name read as words; a
 *  bucket is named by its subject. */
function memoryLabel(path: string): string {
  const m = /^agents\/memories\/(?:([^/]+)\/)?([^/]*)$/.exec(path);
  if (!m) return base(path);
  if (m[2]) return humanizeSlug(m[2].replace(/\.md$/, ""));
  return m[1] ? `Memories · ${displayCode(m[1])}` : "Memories";
}

/** Glyph and name: a library file as its chip draws it, a lecture by its
 *  title, the agent's own files by their file names. */
function look(entry: InfoEntry, lectureTitle: (id: string) => string | undefined) {
  const { path } = entry;
  if (path.startsWith("agents/memories/")) {
    return { icon: <BookmarkSimple size={ICON} className="text-muted-foreground" />, label: memoryLabel(path) };
  }
  if (entry.target.kind === "route") {
    const Icon = entry.label ? FilmSlate : fileIconFor(base(path));
    return { icon: <Icon size={ICON} className="text-muted-foreground" />, label: entry.label ?? base(path) };
  }
  if (path.startsWith("agents/")) {
    const Icon = fileIconFor(base(path));
    return { icon: <Icon size={ICON} className="text-muted-foreground" />, label: base(path) };
  }
  const file = pathFile(parsedSourceOf(path) ?? path);
  const { Icon, tone } = fileReferenceAppearance(file, path);
  const lecture = /^lectures\/([^/]+)$/.exec(path)?.[1];
  const label = lecture
    ? (lectureTitle(lecture) ?? "Lecture")
    : path.endsWith("/")
      ? folderTitle(path)
      : fileTitle(file);
  return { icon: <Icon size={ICON} className={tone} />, label };
}

export function EntryRow({
  entry,
  lectureTitle,
  onOpened,
}: {
  entry: InfoEntry;
  lectureTitle: (id: string) => string | undefined;
  /** After the row opens its thing: the popover closes. */
  onOpened?: () => void;
}) {
  const { icon, label } = look(entry, lectureTitle);
  // A memory or a Studio step is named by what it is; a file says whether
  // the thread made it or changed it.
  const trailing = entry.made
    ? entry.label || entry.path.startsWith("agents/memories/")
      ? undefined
      : entry.made === "new"
        ? "New"
        : "Edited"
    : formatTimes(entry.times) || formatPages(entry.pages);
  return (
    <InfoRow
      icon={icon}
      label={label}
      trailing={trailing}
      title={entry.target.kind === "library" ? entry.target.path : entry.path}
      onOpen={(newTab) => {
        openTarget(entry.target, newTab);
        onOpened?.();
      }}
    />
  );
}
