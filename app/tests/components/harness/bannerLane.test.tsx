import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { Hourglass, Info } from "@phosphor-icons/react";
import { BannerLane, type BannerItem } from "@/components/harness/composer/BannerLane";

const item = (patch: Partial<BannerItem>): BannerItem => ({ id: "a", tone: "neutral", icon: Info, title: "Title", ...patch });

describe("BannerLane", () => {
  test("nothing to say draws nothing", () => {
    expect(renderToStaticMarkup(<BannerLane items={[]} />)).toBe("");
  });

  test("items stack in the order given", () => {
    const html = renderToStaticMarkup(
      <BannerLane items={[item({ id: "first", title: "One" }), item({ id: "second", title: "Two" })]} />,
    );
    expect(html.indexOf('data-banner="first"')).toBeLessThan(html.indexOf('data-banner="second"'));
  });

  test("a card has its title, muted description, pill actions and a close", () => {
    const html = renderToStaticMarkup(
      <BannerLane
        items={[
          item({
            tone: "warning",
            icon: Hourglass,
            title: "Usage limit reached",
            description: "Resets in 2 hr",
            actions: [{ label: "Retry", onClick: () => {}, primary: true }],
            onDismiss: () => {},
          }),
        ]}
      />,
    );
    expect(html).toContain("Usage limit reached");
    expect(html).toContain("text-muted-foreground\">Resets in 2 hr");
    expect(html).toContain("text-warning");
    expect(html).toContain(">Retry</button>");
    expect(html).toContain("rounded-full");
    expect(html).toContain('aria-label="Dismiss: Usage limit reached"');
  });

  test("no close without onDismiss", () => {
    expect(renderToStaticMarkup(<BannerLane items={[item({})]} />)).not.toContain("Dismiss");
  });
});
