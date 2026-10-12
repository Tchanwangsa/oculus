import { useEffect, useRef } from "react";
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import {
  defaultKeymap,
  history,
  historyField,
  historyKeymap,
  indentWithTab,
  redo,
  undo,
} from "@codemirror/commands";
import { bracketMatching, indentOnInput, indentUnit } from "@codemirror/language";
import { Annotation, Compartment, EditorState, Transaction, type Extension } from "@codemirror/state";
import {
  EditorView,
  ViewPlugin,
  drawSelection,
  gutters,
  highlightSpecialChars,
  keymap,
} from "@codemirror/view";

import { rawCodeChrome } from "@/components/documents/editor/chrome/rawMode";
import { fileLanguage } from "@/components/documents/editor/syntax/codeLanguages";
import { codeHighlight } from "@/components/documents/editor/theme/highlight";
import { onEdit } from "@/lib/menu/editRouting";
import { cn } from "@/lib/utils";

import { codeEditorTheme } from "./codeEditorTheme";
import { detectIndent } from "./indent";
import { textChange } from "./textChange";

/** The undo history and caret a remount picks up again (a tab switch). */
export interface EditorMemory {
  read(): unknown;
  write(json: unknown): void;
}

export interface CodeEditorProps {
  /** The text to show. A later value the editor does not hold replaces it,
   *  outside the undo history: another copy's edit, a re-read file. */
  text: string;
  /** Picks the grammar by extension (`fileLanguage`). */
  filename: string;
  readOnly?: boolean;
  /** Each local change's full text. */
  onChange?: (text: string) => void;
  memory?: EditorMemory;
  label?: string;
  className?: string;
}

/** Marks a change that came in through `text`, which `onChange` skips. */
const fromProps = Annotation.define<boolean>();

/** The Edit menu's Undo and Redo (`lib/menu/editRouting.ts`) step this
 *  editor's history while it has focus; on macOS the menu owns ⌘Z. */
const menuUndo = ViewPlugin.fromClass(
  class {
    off: () => void;
    constructor(view: EditorView) {
      this.off = onEdit((redoing) => {
        if (!view.hasFocus) return false;
        (redoing ? redo : undo)(view);
        return true;
      });
    }
    destroy() {
      this.off();
    }
  },
);

const editable = (readOnly: boolean): Extension => [
  EditorState.readOnly.of(readOnly),
  EditorView.editable.of(!readOnly),
];

/**
 * A source file in a CodeMirror view: Raw mode's monospace look and line
 * numbers (`rawCodeChrome`), the notes' code colours (`codeHighlight`), the
 * grammar from the file's extension loaded on demand, and the indent unit
 * the file already uses. It fills its box and scrolls itself. Remounts per
 * `filename`.
 */
export function CodeEditor({
  text,
  filename,
  readOnly = false,
  onChange,
  memory,
  label,
  className,
}: CodeEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const latest = useRef({ onChange, memory, text, readOnly });
  latest.current = { onChange, memory, text, readOnly };
  const compartments = useRef({ language: new Compartment(), editable: new Compartment() });

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const { language, editable: edit } = compartments.current;
    const initial = latest.current.text;
    // This file's memory: by cleanup, `latest` may already hold the next file's.
    const memory = latest.current.memory;
    const config = {
      extensions: [
        history(),
        drawSelection(),
        highlightSpecialChars(),
        gutters(),
        rawCodeChrome(),
        codeHighlight,
        codeEditorTheme,
        bracketMatching(),
        closeBrackets(),
        indentOnInput(),
        indentUnit.of(detectIndent(initial)),
        EditorState.tabSize.of(4),
        keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
        menuUndo,
        language.of([]),
        edit.of(editable(latest.current.readOnly)),
        EditorView.contentAttributes.of({
          "aria-label": label ?? filename,
          spellcheck: "false",
          autocorrect: "off",
          autocapitalize: "off",
        }),
        EditorView.updateListener.of((u) => {
          if (!u.docChanged) return;
          if (u.transactions.every((tr) => tr.annotation(fromProps))) return;
          latest.current.onChange?.(u.state.doc.toString());
        }),
      ],
    };
    let state: EditorState | null = null;
    const saved = memory?.read() as { doc?: string } | null | undefined;
    if (saved && saved.doc === initial) {
      try {
        state = EditorState.fromJSON(saved, config, { history: historyField });
      } catch {
        // An unreadable memory is only a lost undo history.
      }
    }
    const view = new EditorView({
      state: state ?? EditorState.create({ doc: initial, ...config }),
      parent: host,
    });
    viewRef.current = view;

    let live = true;
    const desc = fileLanguage(filename);
    if (desc?.support) {
      view.dispatch({ effects: language.reconfigure(desc.support) });
    } else if (desc) {
      desc
        .load()
        .then((support) => live && view.dispatch({ effects: language.reconfigure(support) }))
        .catch((e) => console.warn(`No ${desc.name} grammar:`, e));
    }
    return () => {
      live = false;
      memory?.write(view.state.toJSON({ history: historyField }));
      view.destroy();
      viewRef.current = null;
    };
    // The view lives per file; `text` and `readOnly` follow in the effects below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filename]);

  useEffect(() => {
    viewRef.current?.dispatch({ effects: compartments.current.editable.reconfigure(editable(readOnly)) });
  }, [readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const doc = view.state.doc.toString();
    if (doc === text) return;
    view.dispatch({
      changes: textChange(doc, text),
      annotations: [fromProps.of(true), Transaction.addToHistory.of(false)],
    });
  }, [text]);

  return <div ref={hostRef} data-selectable className={cn("h-full min-h-0 overflow-hidden", className)} />;
}
