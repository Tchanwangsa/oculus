import { afterAll, expect, spyOn, test } from "bun:test";
import { Database as SQLite } from "bun:sqlite";
import type Database from "@tauri-apps/plugin-sql";
import * as connection from "@/lib/db/connection";
import { upsertFile } from "@/lib/db/files";

const sqlite = new SQLite(":memory:");
sqlite.exec(`CREATE TABLE files (
  subject_id INTEGER, filename TEXT, relative_path TEXT, file_type TEXT,
  size_bytes INTEGER, category TEXT, canvas_id INTEGER, source_url TEXT,
  first_seen_at TEXT, scraped_at TEXT, source_created_at TEXT, source_updated_at TEXT,
  UNIQUE(subject_id, relative_path)
)`);
const db = spyOn(connection, "getDb").mockResolvedValue({
  execute: async (sql: string, bindings: (string | number | null)[]) => {
    const result = sqlite.query(sql).run(...bindings);
    return { rowsAffected: result.changes };
  },
} as unknown as Database);
afterAll(() => { db.mockRestore(); sqlite.close(); });

test("source dates backfill existing rows, update independently, and survive undated upserts", async () => {
  const path = "courses/SUBJ/files/slides.pdf";
  await upsertFile(1, "slides.pdf", path, "pdf", 10, "file");
  sqlite.exec("UPDATE files SET first_seen_at = 'first'");
  const row = () => sqlite.query("SELECT first_seen_at, source_created_at, source_updated_at FROM files WHERE relative_path = ?").get(path);
  expect(row()).toEqual({ first_seen_at: "first", source_created_at: null, source_updated_at: null });
  const created = "2026-10-01T00:00:00.000000000Z";
  const updated = "2026-10-03T00:00:00.000000000Z";
  await upsertFile(1, "slides.pdf", path, "pdf", 10, "file", 1, undefined, created, updated);
  expect(row()).toEqual({ first_seen_at: "first", source_created_at: created, source_updated_at: updated });
  await upsertFile(1, "slides.pdf", path, "pdf", 10, "file");
  expect(row()).toEqual({ first_seen_at: "first", source_created_at: created, source_updated_at: updated });
  const newer = "2026-10-05T00:00:00.000000000Z";
  await upsertFile(1, "slides.pdf", path, "pdf", 10, "file", 1, undefined, null, newer);
  expect(row()).toEqual({ first_seen_at: "first", source_created_at: created, source_updated_at: newer });
});

test("student-made documents, drawings and uploads leave source dates unknown", async () => {
  for (const category of ["document", "canvas", "upload"]) {
    const path = `courses/SUBJ/${category}/own.md`;
    await upsertFile(1, "own.md", path, "md", 10, category);
    expect(sqlite.query("SELECT source_created_at, source_updated_at FROM files WHERE relative_path = ?").get(path))
      .toEqual({ source_created_at: null, source_updated_at: null });
  }
});
