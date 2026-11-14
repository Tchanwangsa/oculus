import { describe, expect, test } from "bun:test";

import { composerCommands, searchSlashItems } from "@/lib/harness/composerCommands";
import { dedupeSkillsByName, searchSkills, splitSkillTokens, type Skill } from "@/lib/harness/skills";

function skill(name: string, more: Partial<Skill> = {}): Skill {
  return { name, description: "", path: `/tmp/${name}/SKILL.md`, source: "oculus", userInvocable: true, ...more };
}

describe("searchSkills", () => {
  // T3's providerSkillSearch fixtures.
  test("uses fuzzy ranking for abbreviated queries", () => {
    const skills = [skill("gh-fix-ci"), skill("github"), skill("agent-browser")];
    expect(searchSkills(skills, "gfc").map((s) => s.name)).toEqual(["gh-fix-ci"]);
  });

  test("ranks a name match before a description match", () => {
    const skills = [
      skill("notes", { description: "Writes a plan for the week" }),
      skill("oculus-plan", { description: "Proposes tasks" }),
    ];
    expect(searchSkills(skills, "plan").map((s) => s.name)).toEqual(["oculus-plan", "notes"]);
  });

  test("an empty query keeps the catalogue order", () => {
    const skills = [skill("b"), skill("a")];
    expect(searchSkills(skills, "/").map((s) => s.name)).toEqual(["b", "a"]);
  });

  test("matches the source badge last", () => {
    const skills = [skill("mine", { source: "personal" }), skill("ours")];
    expect(searchSkills(skills, "personal").map((s) => s.name)).toEqual(["mine"]);
  });
});

describe("dedupeSkillsByName", () => {
  test("returns the first definition for each name", () => {
    const skills = [
      skill("branch-audit", { path: "/lib/agents/.claude/skills/branch-audit/SKILL.md" }),
      skill("browser"),
      skill("branch-audit", { path: "/home/.claude/skills/branch-audit/SKILL.md", source: "personal" }),
    ];
    expect(dedupeSkillsByName(skills).map((s) => s.path)).toEqual([
      "/lib/agents/.claude/skills/branch-audit/SKILL.md",
      "/tmp/browser/SKILL.md",
    ]);
  });
});

describe("searchSlashItems", () => {
  const skills = [skill("oculus-plan", { description: "Proposes tasks" }), skill("models-of-mind")];

  test("lists every command, then every skill, for a bare slash", () => {
    const found = searchSlashItems(composerCommands(), skills, "");
    expect(found.commands.map((c) => c.name)).toEqual(["model"]);
    expect(found.skills.map((s) => s.name)).toEqual(["oculus-plan", "models-of-mind"]);
  });

  test("ranks each group on its own", () => {
    const found = searchSlashItems(composerCommands(), skills, "mod");
    expect(found.commands.map((c) => c.name)).toEqual(["model"]);
    expect(found.skills.map((s) => s.name)).toEqual(["models-of-mind"]);
  });

  test("a description reaches a command", () => {
    expect(searchSlashItems(composerCommands(), [], "switch").commands.map((c) => c.name)).toEqual(["model"]);
  });
});

describe("splitSkillTokens", () => {
  const known = new Set(["oculus-plan", "notes"]);

  test("chips only names in the catalogue, standing as words", () => {
    expect(splitSkillTokens("/oculus-plan week 3 and /other /notes", known)).toEqual([
      { kind: "skill", name: "oculus-plan" },
      { kind: "text", text: " week 3 and /other " },
      { kind: "skill", name: "notes" },
    ]);
  });

  test("leaves paths, fractions and glued slashes as text", () => {
    for (const text of ["see /oculus-plan/x", "1/2", "a/oculus-plan", "/oculus-plan."]) {
      expect(splitSkillTokens(text, known)).toEqual([{ kind: "text", text }]);
    }
  });
});
