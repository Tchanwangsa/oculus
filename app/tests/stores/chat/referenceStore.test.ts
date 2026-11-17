import { describe, expect, test } from "bun:test";

import { checkablePaths, missingReferences, type ReferenceLookups } from "@/stores/chat/referenceStore";

const message =
  "compare `./attachments/a.png` with `agents/attachments/gone.pdf`, see `courses/MAST20004/Course/Files/w1.pdf`, " +
  "`courses/MAST20004/Course/Files/deleted.pdf`, `courses/MAST20004/Course/Pages/` and `lectures/abc/transcript.vtt`\n\n`./attachments/b.png`";

describe("checkablePaths", () => {
  test("names each attachment and `@` library path once, data-directory-relative, and nothing else", () => {
    expect(checkablePaths(`${message} again \`./attachments/a.png\``)).toEqual([
      "agents/attachments/a.png",
      "agents/attachments/gone.pdf",
      "courses/MAST20004/Course/Files/w1.pdf",
      "courses/MAST20004/Course/Files/deleted.pdf",
      "courses/MAST20004/Course/Pages/",
      "agents/attachments/b.png",
    ]);
  });
});

describe("missingReferences", () => {
  const calls: string[][] = [];
  const lookups: ReferenceLookups = {
    attachmentsExist: async (paths) => {
      calls.push(paths);
      return paths.map((p) => !p.includes("gone"));
    },
    libraryPresent: async (paths) => {
      calls.push(paths);
      return new Set(paths.filter((p) => !p.includes("deleted")));
    },
  };

  test("marks a gone attachment and a gone `@` file, in one call per kind", async () => {
    const found = await missingReferences(checkablePaths(message), lookups);
    expect(Object.fromEntries(found)).toEqual({
      "agents/attachments/a.png": false,
      "agents/attachments/gone.pdf": true,
      "agents/attachments/b.png": false,
      "courses/MAST20004/Course/Files/w1.pdf": false,
      "courses/MAST20004/Course/Files/deleted.pdf": true,
      "courses/MAST20004/Course/Pages/": false,
    });
    expect(calls).toHaveLength(2);
  });

  test("a lookup that fails marks nothing unresolved", async () => {
    const failing: ReferenceLookups = {
      attachmentsExist: () => Promise.reject(new Error("no backend")),
      libraryPresent: () => Promise.reject(new Error("no db")),
    };
    expect([...(await missingReferences(checkablePaths(message), failing))]).toEqual([]);
  });
});
