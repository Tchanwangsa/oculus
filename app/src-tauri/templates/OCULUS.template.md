# What Oculus is

> **Stub.** Written once by `oculus docs` and never overwritten — fill it in
> and it stays. Until then, treat it as empty rather than as a claim that
> Oculus has no more to it than the outline below.

Oculus is a desktop app that turns a student's university coursework into a
searchable local library. It syncs Canvas, and Ed Discussion and Echo360 where
the university uses them, into
`courses/<code>/`, parses every PDF page, and embeds the page *images* so
search finds a slide by what it shows, not only by the words extracted from
it.

Still to write here:

- The sync model — what each source contributes, what a re-sync costs.
- Retrieval — why page images rather than extracted text, and what that means
  for the kinds of question `search` answers well.
- The three processes and where the database sits.
- What the CLI can and cannot do without the app running.

The repo's `docs/` covers all of this for someone working on Oculus itself;
this file is the version a coursework agent needs.
