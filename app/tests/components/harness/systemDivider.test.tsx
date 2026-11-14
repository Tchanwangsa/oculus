import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { StopCircle } from "@phosphor-icons/react";
import { dividerTone, SystemDivider } from "@/components/harness/timeline/SystemDivider";

describe("SystemDivider", () => {
  test("neutral draws hairlines in the border colour and muted text", () => {
    const html = renderToStaticMarkup(<SystemDivider icon={StopCircle} label="Context compacted" />);
    expect(html).toContain('data-divider-tone="neutral"');
    expect(html).toContain("bg-border");
    expect(html).toContain("Context compacted");
    expect(html).not.toContain("text-destructive");
  });

  test("danger tints the hairlines, the label and the detail", () => {
    const html = renderToStaticMarkup(<SystemDivider label="Context handoff" detail="failed" tone="danger" />);
    expect(html).toContain('data-divider-tone="danger"');
    expect(html).toContain("bg-destructive/30");
    expect(html).toContain("text-destructive");
    expect(html).not.toContain("bg-border");
  });

  test("tones never share a class", () => {
    const n = dividerTone("neutral");
    const d = dividerTone("danger");
    expect(n.rule).not.toBe(d.rule);
    expect(n.label).not.toBe(d.label);
    expect(n.detail).not.toBe(d.detail);
  });

  test("a detail follows the label after a middle dot; an action is a pill", () => {
    const html = renderToStaticMarkup(
      <SystemDivider label="Run interrupted" detail="Stopped by you" action={{ label: "Resume", onClick: () => {} }} />,
    );
    expect(html).toContain("· Stopped by you");
    expect(html).toContain("rounded-full");
    expect(html).toContain(">Resume</button>");
  });

});
