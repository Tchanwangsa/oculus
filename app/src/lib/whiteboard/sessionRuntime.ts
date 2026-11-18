import type { DbFile } from "@/lib/db";
import { parseWhiteboardDocument } from "./model";
import { whiteboardDrafts } from "./drafts";
import { createWhiteboardSessions } from "./sessions";
import { whiteboardAuthority, deleteWhiteboardFile, readWhiteboard, renameWhiteboardFile, saveWhiteboard } from "./files";

export const whiteboardSessions = createWhiteboardSessions<DbFile>({
  validate: (text) => { parseWhiteboardDocument(text); },
  read: readWhiteboard, write: saveWhiteboard, rename: renameWhiteboardFile, remove: deleteWhiteboardFile, authority: whiteboardAuthority,
  drafts: whiteboardDrafts,
});
