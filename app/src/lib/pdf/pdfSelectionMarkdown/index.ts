/**
 * A selection in the PDF viewer, copied as the parsed markdown instead of
 * pdf.js's text layer. The pages' markdown is one document (`parsedDoc`); a
 * page's text-layer text and its markdown are reduced to one comparison form
 * (`normalizeText`: letters and digits) and aligned patience-diff style, so
 * the selection's two ends land in the document and everything between is one
 * slice — whole pages, unrendered ones and text MinerU filed under the
 * neighbouring page included. The slice is snapped outward so maths, figures,
 * tables, links and code come whole and emphasis stays balanced. An end on a
 * page with no markdown, or one too poorly aligned to trust, copies that
 * page's text. The core is pure (tested under bun, which has no DOM); the DOM
 * edge, `clipboard.ts`, reads pdf.js's `.page` / `.textLayer` markup.
 */
export * from "./skeleton";
export { MIN_COVERAGE, align, mapBoundary } from "./align";
export type { Alignment, Match } from "./align";
export { sliceMarkdown } from "./slice";
export * from "./parsedDoc";
export * from "./clipboard";
