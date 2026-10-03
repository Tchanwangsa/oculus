import { describe, expect, test } from "bun:test";
import { codeLanguage } from "../src/components/documents/editor/codeLanguages";

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
