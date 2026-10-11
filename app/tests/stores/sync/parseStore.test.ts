import { beforeEach, describe, expect, test } from "bun:test";
import { useParseStore, type ParseJob } from "@/stores/sync/parseStore";

const failed = (relative_path: string, patch: Partial<ParseJob>): ParseJob => ({
  relative_path,
  subject_id: 1,
  status: "error",
  error: "failed",
  ...patch,
});

beforeEach(() => {
  useParseStore.setState({ statuses: {}, jobs: {}, failures: {}, latch: null });
});

describe("tokenSaved", () => {
  test("frees only the files a missing or rejected token failed, and lifts the latch", () => {
    const { update } = useParseStore.getState();
    update(failed("a.pdf", { kind: "missing_credentials", retryable: false, latching: true }));
    update(failed("b.pdf", { kind: "document", retryable: false, latching: false }));
    update(failed("c.pdf", { kind: "rejected_credentials", retryable: false, latching: true }));
    update(failed("d.pdf", { kind: "offline", retryable: true, latching: false }));
    expect(useParseStore.getState().latch).not.toBeNull();

    useParseStore.getState().tokenSaved();

    const { failures, latch, statuses } = useParseStore.getState();
    expect(latch).toBeNull();
    expect(Object.keys(failures).sort()).toEqual(["b.pdf", "d.pdf"]);
    // The status stays "error" until the sweep re-kicks the file.
    expect(statuses["a.pdf"]).toBe("error");
  });

  test("with nothing failed, only the latch moves", () => {
    useParseStore.setState({ latch: { message: "x", kind: "quota_exhausted", at: 1 } });
    useParseStore.getState().tokenSaved();
    expect(useParseStore.getState().latch).toBeNull();
    expect(useParseStore.getState().failures).toEqual({});
  });
});
