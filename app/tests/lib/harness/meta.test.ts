import { describe, expect, test } from "bun:test";
import { messageAt, parseErrorMeta, parsePermissionMeta, parseToolMeta, type HarnessItem } from "@/lib/harness";

describe("timeline metadata boundaries", () => {
  const row = (meta: string | null) => ({ meta }) as HarnessItem;
  test("tool fields are narrowed without discarding provider-specific input", () => {
    const input = { CommandLine: "oculus read file.pdf" };
    const item = row(JSON.stringify({ kind: "bash", name: "run", input, ok: false, output: "failed" }));
    expect(parseToolMeta(item)).toMatchObject({ kind: "bash", name: "run", input, ok: false, output: "failed" });
    expect(parseToolMeta(item)).toBe(parseToolMeta(item));
    expect(parseToolMeta(row(JSON.stringify({ kind: "future", name: {}, input, ok: "yes", output: {} }))))
      .toEqual({ input });
  });
  test("replacing a committed row invalidates its metadata cache", () => {
    const item = row('{"kind":"read","ok":null}');
    expect(parseToolMeta(item).ok).toBeNull();
    expect(parseToolMeta({ ...item, meta: '{"kind":"read","ok":true}' }).ok).toBe(true);
  });
  test("auth, lecture moments and permission strings retain their own validation", () => {
    const item = row('{"auth":"codex","at":220,"tool":"Bash","action":"command","target":"ls","rule":"command(ls)"}');
    expect(parseErrorMeta(item)).toEqual({ auth: "codex" });
    expect(messageAt(item)).toBe(220);
    expect(parsePermissionMeta(item)).toEqual({ tool: "Bash", action: "command", target: "ls", rule: "command(ls)" });
    expect(parseErrorMeta(row('{"auth":"future"}'))).toEqual({});
    expect(messageAt(row('{"at":"220"}'))).toBeNull();
    expect(parsePermissionMeta(row('{"tool":12,"rule":{}}')).rule).toBeNull();
  });
});
