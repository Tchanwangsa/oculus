import { snippetParts } from "@/lib/harness";

/** A search snippet as text, its `**` hits as bright medium spans. */
export function Snippet({ text }: { text: string }) {
  return (
    <>
      {snippetParts(text).map((part, i) =>
        part.hit ? (
          <span key={i} className="font-semibold text-foreground">
            {part.text}
          </span>
        ) : (
          part.text
        ),
      )}
    </>
  );
}
