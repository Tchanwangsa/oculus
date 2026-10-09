import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { LectureRow } from "../src/components/lectures/LectureRow";
import { dlKey, useLectureDownloads } from "../src/stores/lectureDownloadStore";
import type { Lecture } from "../src/lib/db";
import type { DlProgress } from "../src/lib/lectures";

const lecture = {
  id: "recording", subject_id: 1, title: "Lecture", completed: false,
  date: null, duration_seconds: 3600, video_path: null, progress_seconds: 0,
} as unknown as Lecture;
const noop = () => {};

function render(progress: Record<string, DlProgress>, active: Record<string, boolean> = {}) {
  // Zustand's server hook reads the initial snapshot instead of the live one.
  const snapshot = useLectureDownloads.getInitialState();
  const original = { progress: snapshot.progress, active: snapshot.active };
  snapshot.progress = progress;
  snapshot.active = active;
  try {
    return renderToStaticMarkup(
      <LectureRow lecture={lecture} active={false} onSelect={noop} onDownload={noop} onDelete={noop} onToggleDone={noop} />,
    );
  } finally {
    snapshot.progress = original.progress;
    snapshot.active = original.active;
  }
}

const progress = (source: 1 | 2, phase: DlProgress["phase"]) => ({
  mediaId: lecture.id, source, phase, percent: 43,
}) as DlProgress;

describe("lecture catalogue download controls", () => {
  test("shows the primary source percentage using the source-qualified progress key", () => {
    const html = render({ [dlKey(lecture.id)]: progress(1, "downloading") });
    expect(html).toContain("43%");
    expect(html).toContain('aria-label="Cancel download"');
    expect(html).not.toContain('aria-label="Download video"');
  });

  test("trimming hides cancel after the transfer completes", () => {
    const html = render({ [dlKey(lecture.id)]: progress(1, "trimming") });
    expect(html).not.toContain('aria-label="Cancel download"');
    expect(html).not.toContain("43%");
    expect(html).not.toContain('aria-label="Download video"');
  });

  test("secondary streams and other lectures cannot change the primary download control", () => {
    const html = render({
      [dlKey(lecture.id, 2)]: progress(2, "downloading"),
      [dlKey("other")]: { ...progress(1, "downloading"), mediaId: "other" },
    });
    expect(html).toContain('aria-label="Download video"');
    expect(html).not.toContain('aria-label="Cancel download"');
    expect(html).not.toContain("43%");
  });

  test("an accepted download without its first progress event still offers cancel", () => {
    const html = render({}, { [dlKey(lecture.id)]: true });
    expect(html).toContain('aria-label="Cancel download"');
    expect(html).not.toContain('aria-label="Download video"');
  });
});
