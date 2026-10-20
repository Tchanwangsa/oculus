import { EditorView, ViewPlugin, showTooltip } from "@codemirror/view";

/**
 * A note's sticky toolbar is a `scrollMargins` top, measured from its page
 * scroller. CodeMirror also adds that margin to the editor's own top to decide
 * where a tooltip's anchor is "outside the editor", and parks the tooltip at
 * -10000px there: with the toolbar above the text, the completion list on the
 * first lines opened but never showed. The page scrolls the caret into view
 * itself, so these tooltips never need that clip.
 */
export const unclippedTooltips = ViewPlugin.fromClass(
  class {
    constructor(view: EditorView) {
      this.unclip(view);
    }
    update(update: { view: EditorView }) {
      this.unclip(update.view);
    }
    unclip(view: EditorView) {
      if (!view.state.facet(EditorView.scrollMargins).some((margin) => (margin(view)?.top ?? 0) > 0)) return;
      for (const tooltip of view.state.facet(showTooltip)) if (tooltip && tooltip.clip !== false) tooltip.clip = false;
    }
  },
);
