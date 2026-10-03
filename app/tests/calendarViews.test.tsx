import { describe, expect, mock, test } from "bun:test";
import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { CalEvent } from "../src/lib/calendar";

// Placement does not require the navigation and markdown card behind a chip.
mock.module("../src/components/calendar/EventPopover", () => ({
  EventPopover: ({ children }: { children: ReactNode }) => children,
}));
const { WeekView } = await import("../src/components/calendar/WeekView");
const { MonthView } = await import("../src/components/calendar/MonthView");

const day = new Date(2026, 9, 26);
const at = (hour: number, minute = 0) => new Date(2026, 9, 26, hour, minute);
const colors = new Map([[1, "#5e6ad2"]]);
const event = (id: string, hour: number, endHour?: number, kind: CalEvent["kind"] = "class"): CalEvent => ({
  id, title: id, kind, subjectId: 1, subjectCode: "COMP30026",
  start: at(hour), end: endHour == null ? null : at(endHour), allDay: false,
  location: null, url: null, description: null, lectureId: null,
  localId: null, localSource: null, taskId: null, projectId: null, projectName: null,
});

function week(events: CalEvent[], fullDay = false) {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: { getItem: () => fullDay ? "1" : null },
  });
  try {
    return renderToStaticMarkup(<WeekView anchor={day} events={events} colors={colors} />);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, "localStorage", descriptor);
    else Reflect.deleteProperty(globalThis, "localStorage");
  }
}

function button(html: string, title: string): string {
  const found = html.match(/<button\b[^>]*>[\s\S]*?<\/button>/g)?.find((b) => b.includes(`>${title}</span>`));
  expect(found).toBeDefined();
  return found!;
}

function occurrences(html: string, title: string) {
  return html.split(`>${title}</span>`).length - 1;
}

describe("calendar view placement", () => {
  test("overlapping classes share lanes and a touching class uses the full column", () => {
    const html = week([event("Class A", 9, 11), event("Class B", 10, 12), event("Class C", 12, 13)]);
    expect(button(html, "Class A")).toContain("width:calc(50% - 4px)");
    expect(button(html, "Class B")).toContain("left:calc(50% + 2px)");
    expect(button(html, "Class C")).toContain("width:calc(100% - 4px)");
  });

  test("instants and all-day events appear exactly once across fitted and full-day grids", () => {
    const inside = event("Midday deadline", 12, undefined, "due");
    const outside = { ...event("Late deadline", 23, undefined, "due"), start: at(23, 59) };
    const allDay = { ...event("All-day note", 12, undefined, "note"), allDay: true };
    for (const fullDay of [false, true]) {
      const html = week([inside, outside, allDay], fullDay);
      for (const e of [inside, outside, allDay]) expect(occurrences(html, e.title)).toBe(1);
      expect(button(html, "Midday deadline")).toContain("height:16px");
      expect(button(html, "Late deadline").includes("height:16px")).toBe(fullDay);
      expect(button(html, "All-day note")).not.toContain("height:16px");
    }
  });

  test("a zero-length midnight class stays within the full-day grid", () => {
    const last = { ...event("Midnight class", 23, 23), start: at(23, 59), end: at(23, 59) };
    const html = week([last], true);
    expect(button(html, last.title)).toContain("top:1088px;height:16px");
  });

  test("unmeasured month cells reserve a chip for the hidden-event list", () => {
    const events = Array.from({ length: 6 }, (_, i) => event(`Month event ${i}`, 9 + i, 10 + i));
    const html = renderToStaticMarkup(<MonthView month={day} events={events} colors={colors} />);
    expect(html).toContain("+3 more");
    for (const e of events.slice(0, 3)) expect(occurrences(html, e.title)).toBe(1);
    for (const e of events.slice(3)) expect(occurrences(html, e.title)).toBe(0);
  });
});
