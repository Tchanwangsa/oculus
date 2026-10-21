import { useEffect, useMemo, useState, type ReactNode } from "react";
import type { LanguageSupport } from "@codemirror/language";
import { highlightCode } from "@lezer/highlight";
import { StyleModule } from "style-mod";
import { codeLanguage } from "@/components/documents/editor/syntax/codeLanguages";
import { codeStyle } from "@/components/documents/editor/theme/codeStyle";

/** Longer code stays plain: the parse is synchronous. */
const MAX_CHARS = 30_000;

let mounted = false;

function mountStyle() {
  if (mounted) return;
  mounted = true;
  StyleModule.mount(document, codeStyle.module!);
}

/**
 * A fenced block's code coloured by the note editor's grammars and palette. The
 * fence's tag picks the grammar; an untagged fence is guessed
 * (`codeLanguage`). Plain until the grammar's import lands, and plain for a
 * tag or guess with no grammar.
 */
export function HighlightedCode({ info, code }: { info: string; code: string }) {
  const desc = useMemo(() => codeLanguage(info, code)?.desc ?? null, [info, code]);
  const [, loaded] = useState(0);
  useEffect(() => {
    if (!desc || desc.support) return;
    let live = true;
    desc.load().then(
      () => live && loaded((n) => n + 1),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [desc]);
  const support = desc?.support;
  if (support) mountStyle();
  const painted = useMemo(() => paint(code, support), [code, support]);
  return <>{painted}</>;
}

/** `code` as text and class-carrying spans, or as is when it cannot be parsed. */
export function paint(code: string, support: LanguageSupport | undefined): ReactNode {
  if (!support || code.length > MAX_CHARS) return code;
  const out: ReactNode[] = [];
  highlightCode(
    code,
    support.language.parser.parse(code),
    codeStyle,
    (text, classes) =>
      out.push(
        classes ? (
          <span key={out.length} className={classes}>
            {text}
          </span>
        ) : (
          text
        ),
      ),
    () => out.push("\n"),
  );
  return out;
}
