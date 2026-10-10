import { useMemo, useRef, type RefObject } from "react";

import { useDataDir } from "@/hooks/backend/useDataDir";
import { libraryImageSrc } from "@/lib/files/libraryLinks";
import { openNoteLink } from "@/lib/files/openFile";
import type { DbFile } from "@/lib/db";

import type { NoteHost } from "../editor/core/host";

/** What the editor asks of its page: picture sources, link opening, the
 *  note's subject and path. */
export function useNoteHost(file: DbFile, files: DbFile[], fileRef: RefObject<DbFile>) {
  const filesRef = useRef(files);
  filesRef.current = files;

  // Pictures resolve against the note's folder, which a rename keeps.
  const dataDir = useDataDir();
  const noteDir = file.relative_path.replace(/[^/]+$/, "");
  const host = useMemo<NoteHost>(
    () => ({
      imageSrc: (src) => libraryImageSrc(src, noteDir, dataDir),
      openLink: (href) => openNoteLink(href, filesRef.current),
      subjectId: file.subject_id,
      // Read when `@` searches, so a rename needn't reconfigure the host.
      get notePath() {
        return fileRef.current.relative_path;
      },
    }),
    [noteDir, dataDir, file.subject_id, fileRef],
  );
  const hostRef = useRef(host);
  hostRef.current = host;

  return { host, hostRef };
}
