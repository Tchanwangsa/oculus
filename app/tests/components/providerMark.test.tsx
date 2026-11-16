import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { CLAUDE_FILL, ProviderMark } from "@/components/icons/ProviderMark";
import type { Provider } from "@/lib/harness";

const mark = (provider: Provider, mono?: boolean) =>
  renderToStaticMarkup(<ProviderMark provider={provider} className="size-4" mono={mono} />);

describe("ProviderMark in colour", () => {
  test("Claude is its path filled in the vendor's clay", () => {
    const html = mark("claude");
    expect(html).toContain(`fill="${CLAUDE_FILL}"`);
    expect(html).not.toContain("currentColor");
  });

  test("OpenAI (Codex) stays currentColor, at full foreground", () => {
    const html = mark("codex");
    expect(html).toContain('fill="currentColor"');
    expect(html).toMatch(/class="size-4 text-foreground"/);
  });

  test("Antigravity is the vendor file as an image, so no ids are inlined", () => {
    const html = mark("antigravity");
    expect(html).toMatch(/<img[^>]*src="[^"]*antigravity\.svg"/);
    expect(html).not.toContain("<svg");
    expect(html).not.toContain("mask0_");
  });

  test("opencode shows the light file in light mode and the dark file under .dark", () => {
    const html = mark("opencode");
    expect(html).toMatch(/<img[^>]*src="[^"]*opencode-light\.svg"[^>]*class="[^"]*dark:hidden/);
    expect(html).toMatch(/<img[^>]*src="[^"]*opencode-dark\.svg"[^>]*class="[^"]*hidden dark:block/);
    expect(html).not.toContain("fill-opacity");
  });

  test("a caller's size reaches every mark", () => {
    for (const p of ["claude", "codex", "antigravity", "opencode"] as Provider[]) expect(mark(p)).toContain("size-4");
  });
});

describe("ProviderMark mono", () => {
  test("every provider draws in currentColor with no vendor colour or image", () => {
    for (const p of ["claude", "codex", "antigravity", "opencode"] as Provider[]) {
      const html = mark(p, true);
      expect(html).toContain('fill="currentColor"');
      expect(html).not.toContain("<img");
      expect(html).not.toContain(CLAUDE_FILL);
      expect(html).not.toContain("text-foreground");
    }
  });

  test("mono opencode keeps one SVG for both themes", () => {
    const html = mark("opencode", true);
    expect(html).toContain('fill-opacity="0.35"');
    expect(html).not.toContain("dark:");
  });
});
