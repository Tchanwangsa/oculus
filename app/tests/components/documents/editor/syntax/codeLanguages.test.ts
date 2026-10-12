import { describe, expect, test } from "bun:test";
import { codeLanguage, fileLanguage } from "@/components/documents/editor/syntax/codeLanguages";
import { CODE_EXTS, isCodeFile } from "@/lib/files/fileTypes";

describe("fenced code language selection", () => {
  test("explicit aliases and unknown tags retain their labels", () => {
    expect(codeLanguage("py", "anything")).toMatchObject({ label: "Python", auto: false });
    expect(codeLanguage("matlab", "anything")).toMatchObject({ label: "MATLAB", auto: false });
    expect(codeLanguage("private-language", "anything")).toMatchObject({ desc: null, label: "private-language", auto: false });
    expect(codeLanguage("text", "def f(): return True")).toBeNull();
    expect(codeLanguage("mermaid", "flowchart TD; A --> B")).toMatchObject({ desc: null, label: "mermaid" });
  });

  test("eligible untagged snippets still choose the same familiar languages", () => {
    expect(codeLanguage("", "def greet(name):\n    print(name)\n    return True")).toMatchObject({ label: "Python", auto: true });
    expect(codeLanguage("", 'const greet = (name) => { console.log(name); return null; };')).toMatchObject({ label: "JavaScript", auto: true });
    expect(codeLanguage("", '#include <iostream>\nint main() { std::cout << "Hello" << std::endl; return 0; }')).toMatchObject({ label: "C++", auto: true });
    expect(codeLanguage("", 'CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR(255) NOT NULL);\nINSERT INTO users VALUES (1, 2);\nSELECT name FROM users WHERE id = 1;')).toMatchObject({ label: "SQL", auto: true });
  });

  test("short and prose fences stay plain, including repeat cache reads", () => {
    for (const source of ["x = 1", "The meeting has several topics to discuss and a list of examples to review."]) {
      expect(codeLanguage("", source)).toBeNull();
      expect(codeLanguage("", source)).toBeNull();
    }
  });
});

describe("file language by name", () => {
  test("picks the grammar from the extension, in any case and below folders", () => {
    expect(fileLanguage("tally.py")?.name).toBe("Python");
    expect(fileLanguage("src/Main.java")?.name).toBe("Java");
    expect(fileLanguage("solution.C")?.name).toBe("C");
    expect(fileLanguage("analysis.R")?.name).toBe("R");
    expect(fileLanguage("notes.txt")).toBeNull();
    expect(fileLanguage("Makefile")).toBeNull();
  });

  test("a course's .m is MATLAB, not Mathematica", () => {
    expect(fileLanguage("fit.m")?.name).toBe("Octave");
  });

  test("every extension the code viewer claims has a grammar", () => {
    for (const ext of CODE_EXTS) expect([ext, fileLanguage(`a.${ext}`)?.name ?? null]).not.toEqual([ext, null]);
  });

  test("markdown and pages stay with the markdown viewer", () => {
    expect(isCodeFile("tally.py")).toBe(true);
    expect(isCodeFile("README.md")).toBe(false);
    expect(isCodeFile("page.html")).toBe(false);
    expect(isCodeFile("marks.csv")).toBe(false);
  });
});
