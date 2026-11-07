/** The tallest a widget's frame may be, in CSS px: about one screen of the
 *  chat at zoom 1. Taller content belongs on the kit's `Pages`; past this
 *  the frame is clipped and the widget shows an overflow error. */
export const WIDGET_HEIGHT_BUDGET = 560;

/** A ```widget fence's class on its `<code>`. */
export const WIDGET_LANG = /(^|\s)language-widget(\s|$)/;
