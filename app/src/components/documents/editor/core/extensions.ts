import { autocompletion } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { indentUnit } from "@codemirror/language";
import { Compartment, Prec, type Extension } from "@codemirror/state";
import {
  EditorView,
  drawSelection,
  dropCursor,
  keymap,
  placeholder,
  type ViewUpdate,
} from "@codemirror/view";

import { imageFiles } from "@/lib/harness/attachments";

import { suggestCompartment } from "../chrome/aiSuggest";
import { noteKeymap } from "../commands";
import { findExtension } from "../chrome/find";
import { hostCompartment, linkAt, noteHost, type NoteHost } from "./host";
import { noteMarkdown } from "./language";
import { livePreview } from "../live-preview/livePreview";
import { mathShorthand } from "../math/tools/shorthand";
import { mathCompletionSource, mathOptionPreview, mathTools } from "../math/tools/mathTools";
import { mentionCompletionSource, mentionOptionClass, mentionOptionIcon } from "../chrome/mentions";
import { rawMode } from "../chrome/rawMode";
import { noteHighlight, noteTheme } from "../theme";
import { noteUndoRouting } from "./undoRouting";

/** Live decorations, or Raw's code-editor look in their place. */
export const liveCompartment = new Compartment();

export function modeExtension(live: boolean): Extension {
  return live ? livePreview() : rawMode();
}

/** ⌘/Ctrl-click opens a link in either mode; a plain click edits it. */
const linkClicks = EditorView.domEventHandlers({
  mousedown(e, view) {
    if (e.button !== 0 || !(e.metaKey || e.ctrlKey)) return false;
    const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
    const href = pos == null ? null : linkAt(view.state, pos);
    if (!href) return false;
    e.preventDefault();
    view.state.facet(noteHost).openLink(href);
    return true;
  },
});

/** Everything a note editor carries. `onUpdate` sees every view update;
 *  `onPictures` takes pasted images and says whether it handled them;
 *  `placeholder` and `label` default to a note's. */
export function noteExtensions(opts: {
  live: boolean;
  host: NoteHost;
  /** `suggestExtension(...)`; the toggle reconfigures `suggestCompartment`. */
  suggest: Extension;
  onUpdate: (update: ViewUpdate) => void;
  onPictures: (files: File[]) => boolean;
  placeholder?: string;
  label?: string;
}): Extension {
  return [
    history(),
    noteUndoRouting(),
    drawSelection(),
    dropCursor(),
    indentUnit.of("  "),
    EditorView.lineWrapping,
    placeholder(opts.placeholder ?? "Start writing…"),
    EditorView.contentAttributes.of({ "aria-label": opts.label ?? "Document text", spellcheck: "true" }),
    noteMarkdown(),
    noteHighlight,
    noteTheme,
    findExtension(),
    // Snippet fields' Tab is Prec.highest (autocomplete), so it beats this
    // Tab while a palette or completion snippet is active.
    Prec.high(keymap.of(noteKeymap)),
    // Before the default keymap, whose Esc would take the popover's.
    mathTools(),
    mathShorthand(),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    // The editor's one completion; add sources to `override`.
    autocompletion({
      override: [mathCompletionSource, mentionCompletionSource],
      activateOnTyping: true,
      closeOnBlur: true,
      icons: false,
      addToOptions: [mathOptionPreview, mentionOptionIcon],
      optionClass: mentionOptionClass,
    }),
    hostCompartment.of(noteHost.of(opts.host)),
    liveCompartment.of(modeExtension(opts.live)),
    suggestCompartment.of(opts.suggest),
    linkClicks,
    EditorView.domEventHandlers({
      paste(e) {
        // A picture beats any text flavour on the clipboard.
        const pictures = imageFiles(e.clipboardData?.files);
        if (!pictures.length) return false;
        e.preventDefault();
        return opts.onPictures(pictures);
      },
    }),
    EditorView.updateListener.of(opts.onUpdate),
  ];
}
