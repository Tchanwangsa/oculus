import type { DrawingKind } from "./drawingKinds";

/**
 * The word text the student reads uses for the drawing in view: "notebook" on a notebook, "whiteboard"
 * on a whiteboard. Where the kind is unknown (a shared page before its first snapshot) the text says
 * "drawing"; where no file is in view (Settings) it says "a whiteboard or notebook".
 */
export const kindNoun = (kind: DrawingKind): "whiteboard" | "notebook" => kind === "notebook" ? "notebook" : "whiteboard";

/** `kindNoun` when the kind is known, else "drawing". */
export const drawingNoun = (kind: DrawingKind | null): "whiteboard" | "notebook" | "drawing" => kind ? kindNoun(kind) : "drawing";

/** `kindNoun`, capitalised to start a sentence or a label. */
export const kindTitle = (kind: DrawingKind): "Whiteboard" | "Notebook" => kind === "notebook" ? "Notebook" : "Whiteboard";

/** A drawing's kind by its file name's extension alone, for code that holds only a path. */
export const kindOfFileName = (path: string): DrawingKind => path.endsWith(".notebook") ? "notebook" : "whiteboard";
