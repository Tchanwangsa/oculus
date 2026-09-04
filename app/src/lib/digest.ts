/**
 * Turning a list of files into summaries, and delivering them to the Inbox.
 *
 * Orchestrated here rather than in Rust because of timing. A scrape reports
 * complete as soon as the bytes are on disk, but a PDF has no text until the
 * sidecar has parsed it — seconds to minutes later, signalled by parse events
 * that already land in the frontend. So the Inbox item is created immediately
 * in `pending` (visible, honest) and each entry fills in as its file becomes
 * readable. Rust only does the model call (`llm_summarize`).
 *
 * Nothing here decides *which* files to summarise or what to ask of them —
 * that comes down the wire from the automation graph.
 */

import { invoke } from "@tauri-apps/api/core";
import {
  addInboxEntry,
  createInboxItem,
  getInboxEntries,
  setInboxEntryResult,
  setInboxItemStatus,
  type DbInboxEntry,
  type DbInboxItem,
} from "@/lib/db";
import { isPdfBacked } from "@/lib/fileTypes";
import type { FileRef } from "@/lib/automations";
import { useInboxStore } from "@/stores/inboxStore";

/** How long to wait for a file's parse before giving up on summarising it. */
const PARSE_WAIT_MS = 15 * 60_000;
const PARSE_POLL_MS = 15_000;

/** Guards against two runs summarising the same Inbox item concurrently. */
const inFlight = new Set<number>();

/** Whether this file's text can be read yet. Markdown-native files (Canvas
 *  pages, announcements, Ed threads) are readable the moment they are
 *  scraped; PDF-backed ones need the parser to have run at least once. */
async function textReady(relativePath: string): Promise<boolean> {
  if (!isPdfBacked(relativePath)) return true;
  const rows = await invoke<[string, string][]>("scan_parsed_files", {
    relativePaths: [relativePath],
  }).catch(() => [] as [string, string][]);
  return rows.length > 0;
}

/** Wait for the file's text, then summarise it. `null` means it never became
 *  readable in time — a skip, not an error. */
async function summariseOne(
  relativePath: string,
  instruction: string,
  deadline: number,
): Promise<string | null> {
  for (;;) {
    if (await textReady(relativePath)) break;
    if (Date.now() > deadline) return null;
    await new Promise((r) => setTimeout(r, PARSE_POLL_MS));
  }
  return invoke<string>("llm_summarize", {
    relativePath,
    instruction: instruction.trim() || null,
  });
}

/**
 * Summarise files without involving the Inbox — what a prompt or a
 * notification body gets when a summaries wire is read as text.
 *
 * Sequential on purpose: a local model holds one set of weights, so parallel
 * calls just queue behind each other with more memory in flight.
 */
export async function materialiseSummaries(
  files: FileRef[],
  instruction: string,
): Promise<{ file: FileRef; text: string }[]> {
  const deadline = Date.now() + PARSE_WAIT_MS;
  const out: { file: FileRef; text: string }[] = [];
  for (const file of files) {
    const text = await summariseOne(file.relativePath, instruction, deadline).catch(
      (e) => String(e),
    );
    if (text != null) out.push({ file, text });
  }
  return out;
}

/** Fill every still-pending entry of an item, one at a time, then close it
 *  out. An entry that never parses is marked `skipped`, not left spinning. */
async function fillEntries(item: DbInboxItem): Promise<void> {
  if (inFlight.has(item.id)) return;
  inFlight.add(item.id);
  try {
    const deadline = Date.now() + PARSE_WAIT_MS;
    const instruction = item.instruction ?? "";
    for (const entry of await getInboxEntries(item.id)) {
      if (entry.status !== "pending") continue;
      await fillEntry(entry, instruction, deadline);
      await useInboxStore.getState().refresh();
    }
    await setInboxItemStatus(item.id, "ready");
    await useInboxStore.getState().refresh();
  } finally {
    inFlight.delete(item.id);
  }
}

async function fillEntry(
  entry: DbInboxEntry,
  instruction: string,
  deadline: number,
): Promise<void> {
  try {
    const text = await summariseOne(entry.relative_path, instruction, deadline);
    if (text == null) {
      await setInboxEntryResult(entry.id, "skipped", "Not parsed in time — no summary.");
    } else {
      await setInboxEntryResult(entry.id, "ready", text);
    }
  } catch (e) {
    await setInboxEntryResult(entry.id, "error", String(e));
  }
}

/**
 * One Inbox item, one row per file, filled in as the summaries land.
 *
 * The rows go in before any model call so the item is on screen while it
 * works — and so a quit mid-way leaves something to resume from. The
 * instruction is stored on the item for exactly that reason: a resumed fill
 * has to ask the same question the graph asked.
 */
export async function deliverSummaries(
  title: string,
  files: FileRef[],
  instruction: string,
  runId: number | null,
): Promise<number | null> {
  if (files.length === 0) return null;

  const itemId = await createInboxItem("summaries", title, runId, instruction);
  for (const f of files) {
    await addInboxEntry(itemId, {
      subjectId: f.subjectId,
      subjectCode: f.subjectCode,
      relativePath: f.relativePath,
      filename: f.filename,
      action: f.action,
    });
  }
  await useInboxStore.getState().refresh();
  await fillEntries({ id: itemId, instruction } as DbInboxItem);
  return itemId;
}

/** Finish items interrupted by the app closing mid-run. */
export async function resumePendingDigests(items: DbInboxItem[]): Promise<void> {
  for (const item of items) {
    await fillEntries(item).catch(() => {});
  }
}
