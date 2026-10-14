import { describe, expect, test } from "bun:test";

import { edUnavailableReason, workspaceStatusText } from "@/lib/ed/workspaceStatus";

describe("workspace status line", () => {
  test("a write in flight, then a failed one, come first", () => {
    expect(workspaceStatusText("offline", { state: "saving" }, "Mac")).toBe("Saving…");
    expect(workspaceStatusText("online", { state: "error", message: "x" }, "Mac")).toBe("Not saved to Ed");
  });

  test("work kept only on this computer says so", () => {
    expect(workspaceStatusText("offline", { state: "idle" }, "Mac")).toBe("Offline — saved on this Mac");
    expect(workspaceStatusText("online", { state: "saved", where: "local" }, "PC")).toBe("Offline — saved on this PC");
  });

  test("otherwise the connection, else everything saved", () => {
    expect(workspaceStatusText("connecting", { state: "idle" }, "Mac")).toBe("Connecting…");
    expect(workspaceStatusText("connecting", { state: "saved", where: "ed" }, "Mac")).toBe("Connecting…");
    expect(workspaceStatusText("online", { state: "idle" }, "Mac")).toBe("All changes saved");
    expect(workspaceStatusText("online", { state: "saved", where: "ed" }, "Mac")).toBe("All changes saved");
  });
});

describe("why Run and Mark are off", () => {
  test("online has no reason; otherwise the connection says why", () => {
    expect(edUnavailableReason({ state: "online", reason: null })).toBeNull();
    expect(edUnavailableReason({ state: "connecting", reason: null })).toBe("Connecting to Ed…");
    expect(edUnavailableReason({ state: "offline", reason: "no network" })).toBe("Offline: no network");
    expect(edUnavailableReason({ state: "offline", reason: null })).toBe("Offline — this needs a connection to Ed");
  });
});
