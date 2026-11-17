import { fullCitation, parsedSourceOf } from "@/lib/citations/parse";
import { videoHref } from "@/lib/studio/routes";
import { RECORDINGS_PREFIX, recordingRel } from "@/lib/files/libraryLayout";

/** What a Thread info row opens: a library path the way a file chip opens
 *  it (`openCitation`), or an app route beside the chat. */
export type InfoTarget =
  | { kind: "library"; path: string; page?: number; at?: number }
  | { kind: "route"; href: string };

/** Where an agent's own instructions, skills and memories are read from;
 *  reading them is how every turn starts, not what the thread read. */
const READ_ROOTS = /^(?:courses\/|lectures\/|agents\/(?:attachments|outputs)\/)/;

/** What an agent can make: its outputs, its memories, and (from a thread
 *  rooted at the library) course files. */
const MADE_ROOTS = /^(?:courses\/|agents\/(?:outputs|memories)\/)/;

/** Studio's folders in the data directory (`docs/studio.md`): a file in a
 *  video's folder opens that video, a library component or theme its page. */
const STUDIO = /\/com\.tchan\.oculus[^/]*\/studio\/(videos|components|themes)\/([^/]+)/;

/** The one key a file is known by: a parsed `.md` is the PDF or Office file
 *  it came from, and anything under a lecture's folder is the lecture. */
export function libraryKey(path: string): string {
  const lecture = new RegExp(`${RECORDINGS_PREFIX}([^/#]+)`).exec(path);
  if (lecture) return recordingRel(lecture[1]);
  return parsedSourceOf(path) ?? path;
}

export function isReadPath(path: string): boolean {
  return READ_ROOTS.test(path);
}

export function isMadePath(path: string): boolean {
  return MADE_ROOTS.test(path);
}

/** A Studio file's page, or null outside Studio's folders. */
export function studioHref(raw: string): string | null {
  const m = STUDIO.exec(raw);
  if (!m) return null;
  const [, area, name] = m;
  if (area === "videos") return videoHref(name);
  const bare = name.replace(/\.[^.]+$/, "");
  return `/studio/${area}/${encodeURIComponent(bare)}`;
}

/** A tool's path as a library path, or null when it names nothing the
 *  library knows. */
export function toLibraryPath(raw: string | null | undefined): string | null {
  return fullCitation(raw?.trim())?.path ?? null;
}
