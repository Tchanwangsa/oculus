import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, sep } from "node:path";
import ts from "typescript";

// React matches a child with its previous render by key among its siblings.
// Two siblings with the same key collide in that match: when the key changes,
// one of the old children is neither reused nor removed, so its DOM stays on
// the page and its effects never clean up (docs/frontend.md#gotchas). A task
// page once stacked every task body it had left that way.

const SRC = join(import.meta.dir, "../../src");

function* sources(dir: string): Generator<string> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* sources(path);
    else if (entry.name.endsWith(".tsx")) yield path;
  }
}

/** What a `key` attribute says, as text: `"a"` and `{"a"}` are the same key. */
function keyOf(element: ts.JsxOpeningLikeElement): string | null {
  for (const attr of element.attributes.properties) {
    if (!ts.isJsxAttribute(attr) || attr.name.getText() !== "key" || !attr.initializer) continue;
    const init = attr.initializer;
    if (ts.isStringLiteral(init)) return JSON.stringify(init.text);
    if (ts.isJsxExpression(init) && init.expression) {
      const e = init.expression;
      return ts.isStringLiteralLike(e) ? JSON.stringify(e.text) : e.getText().replace(/\s+/g, " ");
    }
  }
  return null;
}

/** The keys a child can render. A conditional child renders one of its
 *  branches, so each branch is an alternative, never a second sibling. */
function keysOf(node: ts.Node): string[] {
  if (ts.isJsxElement(node)) return keysOf(node.openingElement);
  if (ts.isJsxSelfClosingElement(node) || ts.isJsxOpeningElement(node)) {
    const key = keyOf(node);
    return key === null ? [] : [key];
  }
  if (ts.isJsxExpression(node)) return node.expression ? keysOf(node.expression) : [];
  if (ts.isParenthesizedExpression(node)) return keysOf(node.expression);
  if (ts.isBinaryExpression(node)) return keysOf(node.right);
  if (ts.isConditionalExpression(node)) {
    return [...new Set([...keysOf(node.whenTrue), ...keysOf(node.whenFalse)])];
  }
  return [];
}

function collisions(file: ts.SourceFile): string[] {
  const hits: string[] = [];
  const check = (children: readonly ts.JsxChild[]) => {
    const seen = new Map<string, ts.Node>();
    for (const child of children) {
      for (const key of keysOf(child)) {
        const first = seen.get(key);
        if (first) {
          const at = file.getLineAndCharacterOfPosition(child.getStart(file));
          const was = file.getLineAndCharacterOfPosition(first.getStart(file));
          hits.push(`${at.line + 1}: key=${key} is also on the sibling at line ${was.line + 1}`);
        } else {
          seen.set(key, child);
        }
      }
    }
  };
  const visit = (node: ts.Node) => {
    if (ts.isJsxElement(node) || ts.isJsxFragment(node)) check(node.children);
    ts.forEachChild(node, visit);
  };
  visit(file);
  return hits;
}

test("no two sibling elements share a key", () => {
  const hits: string[] = [];
  for (const path of sources(SRC)) {
    const text = readFileSync(path, "utf8");
    if (!text.includes("key=")) continue;
    const file = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const rel = relative(SRC, path).split(sep).join("/");
    for (const hit of collisions(file)) hits.push(`src/${rel}:${hit}`);
  }
  expect(hits).toEqual([]);
});

test("the scan sees the collision it guards against", () => {
  const source = `
    const a = <div>
      <Body key={task.id} />
      {list && <Checklist />}
      <Files key={task.id} />
    </div>;
    const b = <div>
      {ready ? <A key="x" /> : <B key={"x"} />}
      <C key={\`files-\${id}\`} />
    </div>;
    const c = <div>
      <A key="x" />
      <B key={"x"} />
    </div>;
  `;
  const file = ts.createSourceFile("probe.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const hits = collisions(file);
  expect(hits).toHaveLength(2);
  expect(hits[0]).toContain("key=task.id");
  expect(hits[1]).toContain('key="x"');
});
