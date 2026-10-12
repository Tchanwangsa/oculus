/** Splits `sentence` around each whole-word `word` (any case). */
function parts(sentence: string, word: string): { text: string; match: boolean }[] {
  const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`(?<![\\p{L}\\p{N}])${escaped}(?![\\p{L}\\p{N}])`, "giu");
  const out: { text: string; match: boolean }[] = [];
  let last = 0;
  for (const m of sentence.matchAll(pattern)) {
    const at = m.index ?? 0;
    if (at > last) out.push({ text: sentence.slice(last, at), match: false });
    out.push({ text: m[0], match: true });
    last = at + m[0].length;
  }
  if (last < sentence.length) out.push({ text: sentence.slice(last), match: false });
  return out;
}

/** The sentence a word is said in, the word picked out in the body colour
 *  (`strong`) or only by weight. */
export function SentenceWithWord({ sentence, word, strong }: {
  sentence: string;
  word: string;
  strong: boolean;
}) {
  return (
    <>
      {parts(sentence, word).map((p, i) =>
        p.match ? (
          <span key={i} className={strong ? "font-medium text-foreground" : "font-medium"}>
            {p.text}
          </span>
        ) : (
          <span key={i}>{p.text}</span>
        ),
      )}
    </>
  );
}
