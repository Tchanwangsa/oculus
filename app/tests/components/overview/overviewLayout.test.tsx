import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { BehindWarning } from "@/pages/subject/content/overview/BehindWarning";
import { WeekStrip } from "@/pages/subject/content/overview/WeekStrip";
import { subjectWeeks } from "@/lib/weeks";
import { event, lecture, subject } from "../../lib/weeks/fixtures";

// Classes in the weeks of 5 and 19 Oct leave 12 Oct between them as a break.
const strip = subjectWeeks({
  subjectId: 1, subjects: [subject()], lectures: [], now: new Date("2026-10-20T12:00:00"),
  classes: [event("2026-10-05T10:00:00"), event("2026-10-19T10:00:00")],
});
const noop = () => {};
const labels = (html: string) =>
  [...html.matchAll(/<span class="max-w-full[^"]*">([^<]+)<\/span>/g)].map((m) => m[1]);

function renderStrip(selected = strip.current) {
  return renderToStaticMarkup(
    <WeekStrip
      strip={strip}
      progress={[{ weekKey: strip.weeks[0].key, watched: 1, total: 2 }]}
      selected={selected}
      onSelect={noop}
    />,
  );
}

test("the strip numbers teaching weeks, names the break and counts on after teaching", () => {
  expect(labels(renderStrip())).toEqual(["1", "Break", "2", "+1", "+2", "+3"]);
});

test("the current week is marked, and stays marked when another week is selected", () => {
  const current = /<button[^>]*aria-current="date"[^>]*>/g;
  expect(strip.current).toBe(2);
  for (const selected of [2, 0]) {
    const html = renderStrip(selected);
    const marked = [...html.matchAll(current)];
    expect(marked).toHaveLength(1);
    expect(marked[0][0]).toContain('data-week="2"');
    expect(html.match(/aria-checked="true"[^>]*data-week="(\d+)"/)?.[1]).toBe(String(selected));
  }
});

test("only the selected week takes the tab stop", () => {
  const html = renderStrip(0);
  expect(html.match(/tabindex="0"/g)).toHaveLength(1);
  expect(html).toContain("1 of 2 lectures watched");
});

test("the behind warning shows only while lectures are behind", () => {
  const behind = [lecture("2026-10-05T10:00:00"), { ...lecture("2026-10-19T10:00:00"), title: "Lecture 2" }];
  const shown = renderToStaticMarkup(
    <BehindWarning behind={{ count: 2, lectures: behind }} onCatchUp={noop} />,
  );
  expect(shown).toContain("2</span> lectures behind: Lecture and Lecture 2");
  expect(shown).toContain("Catch up");
  expect(renderToStaticMarkup(<BehindWarning behind={{ count: 0, lectures: [] }} onCatchUp={noop} />)).toBe("");
});
