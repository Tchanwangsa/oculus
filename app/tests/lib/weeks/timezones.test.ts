import { expect, test } from "bun:test";

test("civil weeks survive Melbourne DST and UTC-to-local Monday boundaries", () => {
  const result = Bun.spawnSync([process.execPath, "--eval", `
    import { subjectWeeks, recordingStatus, weekOf } from "./src/lib/weeks";
    import { event, lecture, subject } from "./tests/lib/weeks/fixtures";
    const classes = [event("2026-09-27T14:05:00Z"), event("2026-10-04T13:05:00Z")];
    const strip = subjectWeeks({ subjectId: 1, subjects: [subject()], classes, lectures: [], now: new Date("2026-10-05T00:10:00") });
    console.log(JSON.stringify({
      labels: strip.weeks.map(w => w.label),
      starts: strip.weeks.slice(0, 2).map(w => [w.start.getMonth(), w.start.getDate(), w.start.getHours()]),
      elapsed: (strip.weeks[0].end - strip.weeks[0].start) / 3600000,
      keyGap: (strip.weeks[1].key - strip.weeks[0].key) / 3600000,
      current: strip.current,
      sunday: weekOf(strip, new Date("2026-10-04T23:55:00")) === strip.weeks[0].key,
      monday: weekOf(strip, new Date("2026-10-05T00:00:00")) === strip.weeks[1].key,
      status: recordingStatus(lecture("2026-10-05T00:10:00"), classes),
    }));
  `], { cwd: new URL("../../../", import.meta.url).pathname, env: { ...process.env, TZ: "Australia/Melbourne" } });
  expect(result.exitCode).toBe(0);
  expect(JSON.parse(result.stdout.toString())).toEqual({
    labels: ["1", "2", "+1", "+2", "+3"], starts: [[8, 28, 0], [9, 5, 0]],
    elapsed: 167, keyGap: 168, current: 1, sunday: true, monday: true, status: "timetabled",
  });
});
