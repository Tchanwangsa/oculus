import { beforeEach, describe, expect, test } from "bun:test";
import { editEvent, newEvent, useEventEditor } from "@/stores/planning/eventEditorStore";
import type { CalEvent } from "@/lib/planning/calendar";

beforeEach(() => useEventEditor.setState({ open: false, session: 0, editing: null, day: null }));

function pendingSave() {
  const session = useEventEditor.getState().session;
  let finish!: () => void;
  const write = new Promise<void>((resolve) => { finish = resolve; });
  const completed = write.then(() => useEventEditor.getState().closeSaved(session));
  return { finish, completed };
}

describe("calendar save sessions", () => {
  test("a pending save cannot close a new form reopened for the same day", async () => {
    const day = new Date("2026-10-03T09:00:00Z");
    newEvent(day);
    const old = pendingSave();
    useEventEditor.getState().close();
    newEvent(day);
    const reopened = useEventEditor.getState();
    old.finish();
    await old.completed;
    expect(useEventEditor.getState()).toBe(reopened);
    expect(reopened.open).toBe(true);
    expect(reopened.day).toBe(day);
  });

  test("switching edit targets keeps the new session open when the old save finishes", async () => {
    const first = { id: "local_1", localId: 1 } as CalEvent;
    const second = { id: "local_2", localId: 2 } as CalEvent;
    editEvent(first);
    const old = pendingSave();
    editEvent(second);
    old.finish();
    await old.completed;
    expect(useEventEditor.getState().open).toBe(true);
    expect(useEventEditor.getState().editing).toBe(second);
  });

  test("a completed save closes its current session and clears the editor", async () => {
    newEvent();
    const saved = pendingSave();
    saved.finish();
    await saved.completed;
    expect(useEventEditor.getState().open).toBe(false);
    expect(useEventEditor.getState().editing).toBeNull();
    expect(useEventEditor.getState().day).toBeNull();
  });
});
