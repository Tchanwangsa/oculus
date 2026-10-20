import { expect, test } from "bun:test";
import { mergeIconPreference } from "@/lib/ui/iconPreferences";
import { projectDefaultColor } from "@/stores/planning/projectIconStore";
import { courseColor } from "@/stores/planning/subjectIconStore";

test("project defaults follow the subject colour and Personal stays grey", () => {
  expect(projectDefaultColor(null)).toBe("#8f98a0");
  expect(projectDefaultColor(undefined, "#12a594")).toBe("#8f98a0");
  expect(projectDefaultColor("SUBJ101")).toBe(courseColor("SUBJ101"));
  expect(projectDefaultColor("SUBJ101", "#12a594")).toBe("#12a594");
});

test("changing a project's glyph preserves its explicit colour", () => {
  expect(mergeIconPreference({icon: "Cube", color: "#12a594"}, {icon: "RocketLaunch"}))
    .toEqual({icon: "RocketLaunch", color: "#12a594"});
});

test("Auto removes only the colour override, allowing live subject inheritance", () => {
  const pref = mergeIconPreference({icon: "RocketLaunch", color: "#12a594"}, {color: undefined});
  expect(pref).toEqual({icon: "RocketLaunch"});
  expect(pref?.color ?? projectDefaultColor("SUBJ101", "#e5484d")).toBe("#e5484d");
});

test("clearing every override deletes the preference instead of persisting undefined fields", () => {
  expect(mergeIconPreference({icon: "RocketLaunch"}, {icon: undefined})).toBeUndefined();
  expect(mergeIconPreference({color: "#12a594"}, {icon: undefined})).toEqual({color: "#12a594"});
  expect(mergeIconPreference(undefined, {color: undefined})).toBeUndefined();
});
