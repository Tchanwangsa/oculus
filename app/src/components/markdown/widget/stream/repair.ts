/**
 * Closes a prefix of a widget's TSX so it parses: open strings, templates and
 * comments end, an open JSX tag gets its `>`, and every open bracket and JSX
 * element is closed in the order they were opened (docs/widgets.md, "A
 * widget draws while it streams"). It only appends: the prefix is never
 * edited and nothing after it is guessed. Ported from the stage 6 spike
 * (`data/evals/widget-stream-spike/repair.ts`), where bracket balancing and
 * JSX closing were the two repairs that made prefixes compile.
 *
 * A prefix whose repair would close a loop's body (`for`, `while`, `do`) is
 * refused: the cut may have dropped the line that ends the loop, and a
 * frame's script can't be interrupted.
 */

type Mode = "js" | "line" | "comment" | "quote" | "template" | "text" | "tag";

interface Tag {
  closing: boolean;
  name: string;
  back: Mode;
}

/** An open bracket (`close` is `)`, `]` or `}`) or JSX element (`tag`). */
interface Open {
  close: string;
  back: Mode;
  tag?: string;
  /** For `{` inside a tag: the tag to return to. */
  tagContext?: Tag | null;
  /** For `(`: the word before it (`while`, `for`, a function's name). */
  word?: string;
  /** For `{`: a loop's body. */
  loop?: boolean;
}

const CLOSE: Record<string, string> = { "(": ")", "[": "]", "{": "}" };
const LOOPS = new Set(["for", "while"]);

/** The repaired prefix, or null when it would close a loop's body. */
export function closePrefix(source: string): string | null {
  let mode: Mode = "js";
  let quote = "";
  let quoteBack: Mode = "js";
  let tag: Tag | null = null;
  const stack: Open[] = [];
  // The last significant token: a word, a punctuator, or "value" after a literal.
  let prev = "";
  // The word before the `(` most recently closed: `while (x) {` opens a loop body.
  let parenWord = "";
  const exprStart = () => !prev || /[=([{,:;!?&|>]$/.test(prev) || /\b(return|yield|case)$/.test(prev);
  const pop = (close: string) => {
    const top = stack[stack.length - 1];
    if (top?.close !== close) return;
    stack.pop();
    mode = top.back;
    if (top.tagContext) tag = top.tagContext;
    if (close === ")") parenWord = top.word ?? "";
  };

  for (let i = 0; i < source.length; i++) {
    const c = source[i];
    const n = source[i + 1];
    if (mode === "line") {
      if (c === "\n") mode = quoteBack;
      continue;
    }
    if (mode === "comment") {
      if (c === "*" && n === "/") {
        i++;
        mode = quoteBack;
      }
      continue;
    }
    if (mode === "quote") {
      if (c === "\\") i++;
      else if (c === quote) {
        mode = quoteBack;
        prev = "value";
      }
      continue;
    }
    if (mode === "template") {
      if (c === "\\") i++;
      else if (c === "`") {
        mode = quoteBack;
        prev = "value";
      } else if (c === "$" && n === "{") {
        stack.push({ close: "}", back: "template" });
        mode = "js";
        prev = "";
        i++;
      }
      continue;
    }
    if (mode === "text") {
      if (c === "{") {
        stack.push({ close: "}", back: "text" });
        mode = "js";
        prev = "";
        continue;
      }
      if (c !== "<") continue;
    }
    if (mode === "tag") {
      if (c === '"' || c === "'") {
        quote = c;
        quoteBack = "tag";
        mode = "quote";
        continue;
      }
      if (c === "{") {
        stack.push({ close: "}", back: "tag", tagContext: tag });
        mode = "js";
        prev = "";
        continue;
      }
      if (c === ">" && tag) {
        if (tag.closing) {
          const top = stack[stack.length - 1];
          if (top?.tag === tag.name) {
            stack.pop();
            mode = top.back;
          } else mode = "text";
        } else if (source[i - 1] === "/") mode = tag.back;
        else {
          stack.push({ tag: tag.name, close: `</${tag.name}>`, back: tag.back });
          mode = "text";
        }
        tag = null;
        prev = "value";
      }
      continue;
    }
    if (c === "<" && (mode === "text" || exprStart()) && /^<\/?(?:[A-Za-z][\w.:-]*|>)/.test(source.slice(i, i + 64))) {
      const m = source.slice(i, i + 256).match(/^<(\/)?([A-Za-z][\w.:-]*)?/)!;
      tag = { closing: !!m[1], name: m[2] ?? "", back: mode };
      mode = "tag";
      i += m[0].length - 1;
      continue;
    }
    if (mode === "text") continue;
    if (c === "/" && n === "/") {
      quoteBack = mode;
      mode = "line";
      i++;
      continue;
    }
    if (c === "/" && n === "*") {
      quoteBack = mode;
      mode = "comment";
      i++;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      quoteBack = mode;
      mode = "quote";
      continue;
    }
    if (c === "`") {
      quoteBack = mode;
      mode = "template";
      continue;
    }
    if (c === "/" && exprStart()) {
      // A regex literal on this line: its brackets are not delimiters.
      let j = i + 1;
      let inClass = false;
      for (; j < source.length && source[j] !== "\n"; j++) {
        if (source[j] === "\\") {
          j++;
          continue;
        }
        if (source[j] === "[") inClass = true;
        if (source[j] === "]") inClass = false;
        if (source[j] === "/" && !inClass) break;
      }
      if (source[j] === "/") {
        i = j;
        prev = "value";
        continue;
      }
    }
    if (c === "(" || c === "[" || c === "{") {
      const open: Open = { close: CLOSE[c], back: mode };
      if (c === "(") open.word = prev;
      if (c === "{") open.loop = prev === "do" || (prev === ")" && LOOPS.has(parenWord));
      stack.push(open);
      prev = c;
      continue;
    }
    if (c === ")" || c === "]" || c === "}") {
      pop(c);
      prev = c;
      continue;
    }
    if (/[\w$]/.test(c)) {
      const word = source.slice(i).match(/^[\w$]+/)![0];
      prev = word;
      i += word.length - 1;
      continue;
    }
    if (!/\s/.test(c)) prev = c;
  }

  if (stack.some((open) => open.loop)) return null;
  let suffix = mode === "line" ? "\n" : "";
  // A closing quote goes right after the text; anything else on a new line,
  // so it can't land inside a trailing line comment.
  // A tag cut after its `/` closes as `/>`, which can't be split.
  const selfClosing = mode === "tag" && source.endsWith("/");
  const joiner = mode === "quote" || mode === "template" || selfClosing ? "" : "\n";
  if (mode === "comment") suffix += "*/";
  else if (mode === "quote") {
    suffix += quote;
    mode = quoteBack;
  } else if (mode === "template") {
    suffix += "`";
    mode = quoteBack;
  }
  // `tag` is narrowed to null by the loop's closures; read it through a cast.
  const openTag = tag as Tag | null;
  if (mode === "tag" && openTag) {
    suffix += ">";
    if (!selfClosing && !openTag.closing) stack.push({ tag: openTag.name, close: `</${openTag.name}>`, back: openTag.back });
    else if (!selfClosing && stack[stack.length - 1]?.tag === openTag.name) stack.pop();
  }
  for (const open of [...stack].reverse()) {
    suffix += open.close;
    if (open.tag === undefined && open.back === "template") suffix += "`";
  }
  return source + joiner + suffix;
}
