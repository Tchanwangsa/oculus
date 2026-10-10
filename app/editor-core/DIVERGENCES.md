# Where the parser differs from the specs

The parser's target is `@lezer/markdown` 1.7.2 as the app configures it, not
the CommonMark spec; `tests/spec/main.rs` renders the CommonMark 0.31.2 examples
and the GFM table, strikethrough and task-list examples from the tree and
compares them with the spec's HTML. Every example that differs is listed
here, and the test fails if that set changes in either direction. Each
difference below is Lezer's (or the app's grammar extensions') behaviour,
reproduced on purpose. Ids are CommonMark example numbers, or pulldown-cmark
test names for the GFM examples (see `NOTICE`).

The GFM autolink-extension examples are not checked: no copy of the GFM
spec is on this machine.

## Lezer's CommonMark

- 5: a tab only partly used up by indentation is dropped from indented code (no partial-tab spaces).
- 6: the same, after a block quote marker.
- 7: the same, after a list marker.
- 10: an ATX heading needs a space after the `#`s; a tab does not count.
- 112: a blank line inside indented code keeps no whitespace past the indentation.
- 171: HTML block start condition 1 does not include `textarea`.
- 280: a list item that starts with a blank line takes an indented line after a further blank line, instead of ending empty.
- 493: a `<…>` link destination does not honour backslash escapes, so `\>` closes it.
- 512: any `[…]` is a Link whether or not a definition exists, so `[bar]` inside `[link [foo [bar]]](/uri)` blocks the outer link.
- 523: the same: `[bar* baz]` is a Link, so the emphasis cannot close inside it.
- 528: the same: the inner `[bar]` makes the outer brackets text.
- 569: the same: `[foo][bar]` is a link with label `bar`, defined or not.
- 571: the same.
- 621: an inline HTML tag may have whitespace after `<`.
- 625: an HTML comment may not contain `--`.
- 626: `<!-->` is not a comment, and `<!--->` starts a longer comment that runs to the next `-->`.
- gfm_strikethrough_test_1: a single `~` is not strikethrough.
- gfm_strikethrough_test_3: in a run of three tildes the last two open or close strikethrough.

## The app's grammar extensions

- 12: `\(…\)` is inline maths (`mathSyntax.ts`), so `\(` and `\)` are not escapes.
- 14: a line starting with `\[` opens display maths.
- 563: the same.
- 96: a `---` first line with a later `---` line is frontmatter (`frontmatter.ts`), not a rule and a setext heading.
- 98: the same.
- 602: bare URLs are autolinked (GFM `Autolink`), here inside a rejected `<…>`.
- 608: the same.
- 611: the same.
- 612: the same, for an email address.

## The document model (`src/text/`) against CodeMirror

`oracle/changes.ts` and `oracle/history.ts` check `src/text/` against
`@codemirror/state` 6.7.6 and `@codemirror/commands` 6.11.1. These are the
places where the API differs from CodeMirror's on purpose, then the
CodeMirror behaviours it reproduces even though they look wrong. (Bullets
are `*` so the spec test, which reads `- <id>:` lines, skips them.)

### API differences

* `History::undo`/`redo`/`undo_selection`/`redo_selection` return the
  transaction together with the history after it, instead of tagging the
  transaction with a `fromHistory` annotation. Apply the transaction to the
  state and don't pass it to `History::update`.
* Not ported: history effects (`invertedEffects`), a custom `joinToEvent`,
  `canUndoSelection`/`canRedoSelection`, the `undirectional` range flag,
  `lineSeparator`, transactions built from several specs, and
  `ChangeSet.filter`/`fromJSON`.
* `goal_column` is an `Option<f64>`, because the view stores a pixel offset
  there.
* Bad input returns errors where CodeMirror throws: `ChangeSet::of`, `apply`,
  `invert`, `try_compose`, `try_map`, `State::change_by_range`,
  `replace_selection` and `History::update` and `undo`/`redo` (a transaction for
  another document, or one that splits a surrogate pair). `compose`, `map` and
  `map_pos` panic on mismatched lengths or out-of-range positions.
* A `user_event` of `""` is stored as none, as CodeMirror treats it.

### CodeMirror behaviour kept on purpose

* Mapping a position through `compose(a, b)` can differ from mapping it
  through `a` and then `b` when the position touches a change. Example: `a`
  deletes 2..4 and `b` inserts at 2. Position 2 with assoc 1 maps to 3 step
  by step, but to 2 through the composition, which is a single replacement.
* Mapping a non-empty range over a replacement inside it can leave
  `from > to`. A later `replace_selection` or `change_by_range` over that range
  then fails, as CodeMirror's throws. **The app layer must handle this**
  (CodeMirror's typing command would throw at that point too).
* After an undo or redo `prev_time` is 0, so the next edit never joins the
  group it undid.
* A branch is trimmed only once it is 20 events past `min_depth`, and then
  keeps `min_depth + 2` events.
* The exception: CodeMirror's `mapSet` loops forever when the other set is
  longer and has changes past this set's end. Here every mapping and
  composition checks the lengths first.
