import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { findExtension } from "../editor/chrome/find";
import { noteHost, type NoteHost } from "../editor/core/host";
import { noteMarkdown } from "../editor/core/language";
import { noteSelection } from "../editor/core/selectionLayer";
import { livePreviewRender } from "../editor/live-preview/livePreview";
import { noteHighlight, noteTheme } from "../editor/theme";
import { diffField, versionDiff, type VersionDiff } from "./diffDecorations";
import { diffTheme } from "./diffTheme";

/** The host, swapped in place when the note's path or data dir changes. */
export const versionHost = new Compartment();

/**
 * A version's state: the note's language, theme and Live rendering, with
 * nothing that edits — no history, keymaps, completion or paste, and none
 * of Live mode's editing pieces (`livePreviewRender`). Read-only twice
 * over, and a change filter refuses what widgets dispatch themselves (a
 * task's checkbox), so the shown text can never change.
 */
export function versionState(text: string, diff: VersionDiff | null, host: NoteHost): EditorState {
  const extensions: Extension = [
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
    EditorState.changeFilter.of(() => false),
    EditorView.lineWrapping,
    EditorView.contentAttributes.of({ "aria-label": "Version text", "aria-readonly": "true" }),
    noteSelection(),
    noteMarkdown(),
    noteHighlight,
    noteTheme,
    findExtension(),
    versionHost.of(noteHost.of(host)),
    versionDiff.of(diff),
    diffField,
    diffTheme,
    livePreviewRender(),
  ];
  return EditorState.create({ doc: text, extensions });
}
