import { describe, expect, test } from "bun:test";
import { dropInlineFiles, workspaceFilePath } from "@/lib/ed/briefFiles";

/** A challenge slide's body as `files_md` (sources/ed/lesson_md/body.rs) ends it. */
const BRIEF = [
  "# Tally",
  "",
  "Count the votes in `tally.py`.",
  "",
  "**Files**",
  "",
  "- [tally.py](./03-tally/tally.py)",
  "- [data/votes\\_2024.txt](./03-tally/data/votes_2024.txt)",
  "- [big.csv](./03-tally/big.csv)",
  "",
  "**tally.py**",
  "",
  "````python",
  "def tally():",
  "    \"\"\"```not a fence```\"\"\"",
  "````",
  "",
  "**data/votes\\_2024.txt**",
  "",
  "```text",
  "a",
  "b",
  "```",
  "",
  "_The challenge's files could not be fetched._",
].join("\n");

describe("dropInlineFiles", () => {
  test("drops each listed file's copy and keeps the brief, the list and what follows", () => {
    expect(dropInlineFiles(BRIEF)).toBe(
      [
        "# Tally",
        "",
        "Count the votes in `tally.py`.",
        "",
        "**Files**",
        "",
        "- [tally.py](./03-tally/tally.py)",
        "- [data/votes\\_2024.txt](./03-tally/data/votes_2024.txt)",
        "- [big.csv](./03-tally/big.csv)",
        "",
        "_The challenge's files could not be fetched._",
      ].join("\n"),
    );
  });

  test("leaves a body without a Files list, or a bold block not in the list, alone", () => {
    const plain = "# Notes\n\n**tally.py**\n\n```python\nx\n```\n";
    expect(dropInlineFiles(plain)).toBe(plain);
    const other = "**Files**\n\n- [a.py](./s/a.py)\n\n**b.py**\n\n```python\nx\n```\n";
    expect(dropInlineFiles(other)).toBe(other);
  });

  test("stops at an unclosed fence", () => {
    const open = "**Files**\n\n- [a.py](./s/a.py)\n\n**a.py**\n\n```python\nx";
    expect(dropInlineFiles(open)).toBe(open);
  });
});

describe("workspaceFilePath", () => {
  const brief = "courses/C/lessons/1-l/03-tally.md";
  test("a link into the slide's folder is its workspace path, decoded", () => {
    expect(workspaceFilePath("./03-tally/tally.py", brief)).toBe("tally.py");
    expect(workspaceFilePath("./03-tally/src/my%20file.py", brief)).toBe("src/my file.py");
  });
  test("anything else is not a workspace file", () => {
    expect(workspaceFilePath("./04-other/tally.py", brief)).toBeNull();
    expect(workspaceFilePath("../2-l.md", brief)).toBeNull();
    expect(workspaceFilePath("https://edstem.org/x", brief)).toBeNull();
    expect(workspaceFilePath("./03-tally/%E0%A4%A", brief)).toBeNull();
  });
});
