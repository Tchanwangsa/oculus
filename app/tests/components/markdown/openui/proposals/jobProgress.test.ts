import { describe, expect, test } from "bun:test";
import { jobProgress, matchesJobProgressEvent, type JobProgressSources } from "@/components/markdown/openui/proposals/jobProgress.ts";

const sources = (): JobProgressSources => ({ sync: { runId: 9, scraping: true, progress: { done: 2, total: 8, course: "CS101", phase: "modules" } }, parses: { statuses: {}, jobs: {}, failures: {} } });
describe("job progress follows the confirmed target", () => {
  test("a second sync never inherits the active run's progress or cancel", () => {
    const source = sources();
    expect(jobProgress("Sync", { job: { run_id: 8 } }, source).sync).toBeUndefined();
    expect(jobProgress("Sync", { job: { run_id: 8 } }, source).cancel).toBeUndefined();
    expect(jobProgress("Sync", { job: { run_id: 9 } }, source).sync?.done).toBe(2);
    expect(jobProgress("Sync", { job: { run_id: 9 } }, source).cancel?.command).toBe("cancel_scrape");
    expect(matchesJobProgressEvent("Sync", { job: { run_id: 8 } }, "scrape-progress", { runId: 9 })).toBe(false);
    expect(matchesJobProgressEvent("Sync", {}, "scrape-progress", {})).toBe(false);
  });
  test("tagged lecture refresh progress belongs only to its run", () => {
    const source = sources();
    source.event = { name: "scrape-progress", payload: { runId: 8, course: "CS102", phase: "lectures", done: 1, total: 2 } };
    expect(jobProgress("Sync", { lectures: true, job: { run_id: 8 } }, source).sync?.course).toBe("CS102");
    expect(jobProgress("Sync", { lectures: true, job: { run_id: 8 } }, source).cancel).toBeUndefined();
  });
  test("reparse counts only its files and surfaces a fresh parse failure", () => {
    const source = sources();
    source.parses.statuses = { a: "quality", b: "running", unrelated: "quality" };
    source.parses.jobs.b = { relative_path: "b", subject_id: 1, status: "running" };
    source.parses.failures.c = { message: "Parser rejected this PDF", at: 2000 };
    const value = jobProgress("Reparse", { files: ["a", "b", "c"], job: { started_at: new Date(1000).toISOString() } }, source);
    expect(value.done).toBe(1);
    expect(value.total).toBe(3);
    expect(value.label).toBe("Parsing b");
    expect(value.error).toBe("Parser rejected this PDF");
    expect(value.cancel).toBeUndefined();
    expect(jobProgress("Reparse", { files: ["c"], job: { started_at: new Date(3000).toISOString() } }, source).error).toBeUndefined();
  });
  test("transcription is path scoped and chunks count after they complete", () => {
    const source = sources();
    source.event = { name: "transcribe-progress", payload: { path: "a.mp4", phase: "transcribing", chunk: 2, chunks: 4 } };
    expect(jobProgress("Transcribe", { file: "a.mp4" }, source).percent).toBe(25);
    expect(jobProgress("Transcribe", { file: "b.mp4" }, source).percent).toBeUndefined();
    expect(jobProgress("Transcribe", { file: "a.mp4" }, source).cancel).toBeUndefined();
  });
  test("downloads match their channel and voice, with only existing cancel commands", () => {
    const source = sources();
    source.event = { name: "studio-voice-file-progress", payload: { id: "af_heart", phase: "downloading", received: 5, total: 10 } };
    expect(jobProgress("Download", { item: "af_heart" }, source).percent).toBe(50);
    expect(jobProgress("Download", { item: "aligner" }, source).percent).toBeUndefined();
    expect(jobProgress("Download", { item: "voices" }, source).cancel).toEqual({ command: "studio_voice_cancel", args: { id: "af_heart" } });
    expect(matchesJobProgressEvent("Download", { item: "am_michael" }, source.event.name, source.event.payload)).toBe(false);
    source.event = { name: "studio-aligner-progress", payload: { phase: "downloading", received: 20, total: 10 } };
    expect(jobProgress("Download", { item: "aligner" }, source).percent).toBe(100);
  });
  test("chapter steps match the lecture; lecture end has no invented percentage or cancel", () => {
    const source = sources();
    source.event = { name: "lecture-chapter-progress", payload: { lectureId: "L1", phase: "frames", done: 2, total: 5 } };
    expect(jobProgress("LectureChapters", { lecture: "L1" }, source)).toMatchObject({ label: "Grabbing slide frames", percent: 40 });
    expect(jobProgress("LectureChapters", { lecture: "L2" }, source).percent).toBeUndefined();
    expect(jobProgress("LectureEnd", { lecture: "L1" }, source)).toEqual({ label: "Reading the lecture transcript…" });
  });
});
