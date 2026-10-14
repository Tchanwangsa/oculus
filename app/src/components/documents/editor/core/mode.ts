import { Facet } from "@codemirror/state";

/** Whether the note draws rendered markdown or Raw source. */
export const liveMode = Facet.define<boolean, boolean>({
  combine: (values) => values[0] ?? false,
});
