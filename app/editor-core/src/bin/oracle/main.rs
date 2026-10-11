//! The oracle: one JSON request per line on stdin, one JSON answer per line on
//! stdout. Each request names its area with `"op"`; each area is a module that
//! owns its request types and answers. `oracle/*.ts` sends the same requests to
//! CodeMirror/Lezer and compares. A refused request answers `{"error": …}`.

mod changes;
mod history;
mod markdown;
mod text;

use std::io::{self, BufRead, Write};

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    /// Read-only queries on one document.
    Text(text::Probe),
    /// A sequence of edits on one document, probed after each.
    TextEdits(text::Edits),
    /// The rope's leaf boundaries, so the driver can probe where trees split.
    TextChunks(text::ChunksRequest),
    /// One document's parse tree, with `resolve_inner` probes.
    Markdown(markdown::Query),
    /// Every `ChangeSet` operation on one document and three sets.
    Changes(changes::Request),
    /// An undo history driven through transactions and commands.
    History(history::Request),
}

fn answer(request: Request) -> Result<Value, String> {
    match request {
        Request::Text(probe) => text::probe(probe),
        Request::TextEdits(edits) => text::edits(edits),
        Request::TextChunks(request) => Ok(text::chunks(request)),
        Request::Markdown(query) => markdown::parse(query),
        Request::Changes(request) => changes::answer(request),
        Request::History(request) => history::answer(request),
    }
}

fn main() -> io::Result<()> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line?;
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(request) => answer(request).unwrap_or_else(|error| json!({ "error": error })),
            Err(error) => json!({ "error": format!("bad request: {error}") }),
        };
        serde_json::to_writer(&mut out, &reply).map_err(io::Error::other)?;
        out.write_all(b"\n")?;
    }
    out.flush()
}
