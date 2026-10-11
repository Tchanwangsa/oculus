/** Letters and digits only, lowercased, ligatures and accents folded — the
 *  form a quote and a PDF's text layer are compared in. */
export function normalizeText(s: string): string {
  return s.normalize("NFKD").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
}
