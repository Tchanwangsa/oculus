//! A grid as source text: each cell's text and the text between cells,
//! kept as written. The keys add, remove and move cells here; [`Model::write`]
//! gives back the environment's content, so the change to the source is
//! only what the key did.

use core::ops::Range;

use crate::command::ends_with_word;

/// What sits between two cells, or two rows: the source's own text, or a
/// separator a key added (written in the grid's [`Style`]).
#[derive(Clone, Debug)]
pub enum Piece {
    Kept(String),
    New,
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub cells: Vec<String>,
    /// Between cell `i` and `i + 1`.
    pub seps: Vec<Piece>,
}

/// How new separators are written.
#[derive(Clone, Debug)]
pub struct Style {
    /// Between cells: `" & "` or `"&"`, as the grid's first `&` is.
    pub col: &'static str,
    /// Between rows: the grid's first `\\` with its spaces, else `" \\ "`
    /// on one line (`"\\"` beside a tight `&`), or `" \\"` and a newline
    /// in a display block.
    pub row: String,
    /// The environment is the whole of a display formula and has one row:
    /// once it has two, it opens and closes on lines of their own.
    pub block: bool,
}

/// A grid's content between `\begin{env}` and `\end{env}`.
#[derive(Clone, Debug)]
pub struct Model {
    /// Before the first cell (a column spec, spaces).
    pub lead: String,
    pub rows: Vec<Row>,
    /// Between row `i` and `i + 1`.
    pub breaks: Vec<Piece>,
    /// After the last cell; a `\\` here ends a row KaTeX drops.
    pub tail: String,
    pub style: Style,
}

/// A cell's row and column.
type Cell = (usize, usize);

/// The written content and where each cell landed in it.
pub struct Written {
    pub text: String,
    pub cells: Vec<Vec<Range<usize>>>,
}

impl Model {
    /// The model of `content` (a range of `src`), whose cells are at
    /// `cells` (row, range), in source order.
    pub fn read(src: &str, content: Range<usize>, cells: &[(usize, Range<usize>)]) -> Self {
        let mut rows: Vec<Row> = Vec::new();
        let mut breaks = Vec::new();
        let mut at = content.start;
        let mut lead = String::new();
        for (i, (row, range)) in cells.iter().enumerate() {
            let between = src[at..range.start].to_owned();
            if i == 0 {
                lead = between;
                rows.push(Row::default());
            } else if rows.len() <= *row {
                breaks.push(Piece::Kept(between));
                rows.push(Row::default());
            } else if let Some(last) = rows.last_mut() {
                last.seps.push(Piece::Kept(between));
            }
            if let Some(last) = rows.last_mut() {
                last.cells.push(src[range.clone()].to_owned());
            }
            at = range.end;
        }
        Self {
            lead,
            rows,
            breaks,
            tail: src[at..content.end].to_owned(),
            style: Style {
                col: " & ",
                row: r" \\ ".to_owned(),
                block: false,
            },
        }
    }

    /// A bracket group's body as one cell, its spaces kept around it.
    pub fn group(body: &str) -> Self {
        let cell = body.trim();
        let lead = &body[..body.len() - body.trim_start().len()];
        let tail = &body[body.trim_end().len()..];
        Self {
            lead: lead.to_owned(),
            rows: vec![Row {
                cells: vec![cell.to_owned()],
                seps: Vec::new(),
            }],
            breaks: Vec::new(),
            tail: tail.to_owned(),
            style: Style {
                col: " & ",
                row: r" \\ ".to_owned(),
                block: false,
            },
        }
    }

    /// The first separators written in the source: a key's new ones copy
    /// them.
    pub fn kept_seps(&self) -> (Option<&str>, Option<&str>) {
        fn kept(piece: &Piece) -> Option<&str> {
            match piece {
                Piece::Kept(text) => Some(text),
                Piece::New => None,
            }
        }
        let col = self.rows.iter().flat_map(|row| &row.seps).find_map(kept);
        (col, self.breaks.iter().find_map(kept))
    }

    pub fn width(&self) -> usize {
        self.rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0)
    }

    pub fn row_empty(&self, r: usize) -> bool {
        self.rows[r].cells.iter().all(|cell| cell.trim().is_empty())
    }

    pub fn col_empty(&self, c: usize) -> bool {
        self.rows
            .iter()
            .all(|row| row.cells.get(c).is_none_or(|cell| cell.trim().is_empty()))
    }

    /// One cell left.
    pub fn one_cell(&self) -> bool {
        self.rows.len() == 1 && self.rows[0].cells.len() == 1
    }

    /// Every row as wide as the widest, with empty cells.
    pub fn pad(&mut self) {
        let width = self.width().max(1);
        for row in &mut self.rows {
            while row.cells.len() < width {
                if !row.cells.is_empty() {
                    row.seps.push(Piece::New);
                }
                row.cells.push(String::new());
            }
        }
    }

    /// An empty column after column `c` (in a padded grid).
    pub fn insert_col(&mut self, c: usize) {
        for row in &mut self.rows {
            row.cells.insert(c + 1, String::new());
            row.seps.insert(c, Piece::New);
        }
    }

    /// Column `c` gone (its cells empty).
    pub fn remove_col(&mut self, c: usize) {
        for r in 0..self.rows.len() {
            self.remove_cell(r, c);
        }
    }

    /// Cell `c` of row `r` gone (it is empty). An empty cell's text sits
    /// against the separator after it (`a & |& b`), so that separator goes
    /// with it; a row's last cell takes the one before, whose leading
    /// spaces stay before the row's `\\`.
    pub fn remove_cell(&mut self, r: usize, c: usize) {
        let row = &mut self.rows[r];
        if c >= row.cells.len() {
            return;
        }
        row.cells.remove(c);
        if c < row.seps.len() {
            row.seps.remove(c);
        } else if c > 0 {
            let space = match row.seps.remove(c - 1) {
                Piece::Kept(text) => text[..text.len() - text.trim_start().len()].to_owned(),
                Piece::New => String::new(),
            };
            self.prepend_after_row(r, &space);
        }
    }

    /// `space` at the start of the break after row `r` (`b \\`), if it
    /// has one of its own.
    fn prepend_after_row(&mut self, r: usize, space: &str) {
        if let Some(Piece::Kept(text)) = self.breaks.get_mut(r) {
            text.insert_str(0, space);
        }
    }

    /// A row of `width` empty cells after row `r`.
    pub fn insert_row(&mut self, r: usize, width: usize) {
        let row = Row {
            cells: vec![String::new(); width.max(1)],
            seps: vec![Piece::New; width.max(1) - 1],
        };
        self.rows.insert(r + 1, row);
        self.breaks.insert(r, Piece::New);
    }

    /// Row `r` gone (its cells empty), with the break after it, which its
    /// text sits against; the last row takes the break before it, and the
    /// `\\` after it that kept KaTeX from dropping it.
    pub fn remove_row(&mut self, r: usize) {
        self.rows.remove(r);
        if r < self.breaks.len() {
            self.breaks.remove(r);
        } else if r > 0 {
            self.breaks.remove(r - 1);
            self.drop_tail_break();
        }
    }

    /// The tail without a `\\` (a row KaTeX drops), its spaces after it
    /// kept.
    fn drop_tail_break(&mut self) {
        if let Some(at) = self.tail.rfind(r"\\") {
            self.tail = self.tail[at + 2..].to_owned();
        }
    }

    /// Without trailing rows and columns that are all empty, keeping one,
    /// nor a row KaTeX drops after them.
    pub fn trim(&mut self) {
        self.pad();
        while self.rows.len() > 1 && self.row_empty(self.rows.len() - 1) {
            self.remove_row(self.rows.len() - 1);
        }
        while self.width() > 1 && self.col_empty(self.width() - 1) {
            self.remove_col(self.width() - 1);
        }
        self.drop_tail_break();
    }

    /// Cell `c` of row `r` joined onto the end of the cell before it
    /// (spaced off a control word before a letter), that separator gone.
    pub fn join_back(&mut self, r: usize, c: usize) {
        let row = &mut self.rows[r];
        let moved = row.cells.remove(c);
        row.seps.remove(c - 1);
        let before = row.cells[c - 1].trim_end().to_owned();
        let moved = moved.trim();
        let space =
            if ends_with_word(&before) && moved.starts_with(|c: char| c.is_ascii_alphabetic()) {
                " "
            } else {
                ""
            };
        row.cells[c - 1] = format!("{before}{space}{moved}");
    }

    /// The content as written: kept text as it was, new separators in the
    /// grid's style, each spaced only where its neighbours are not, and
    /// no blank line made.
    pub fn write(&self) -> Written {
        let block = self.style.block && self.rows.len() > 1;
        let edge = |text: &str| {
            if block && text.trim().is_empty() && !text.contains('\n') {
                "\n".to_owned()
            } else {
                text.to_owned()
            }
        };
        let new_or = |piece: &Piece, style: &str| match piece {
            Piece::Kept(text) => (text.clone(), false),
            Piece::New => (style.to_owned(), true),
        };
        // Each piece: its text, whether a key added it, and the cell it is.
        let mut pieces: Vec<(String, bool, Option<Cell>)> = vec![(edge(&self.lead), false, None)];
        for (r, row) in self.rows.iter().enumerate() {
            if r > 0 {
                let (text, new) = new_or(&self.breaks[r - 1], &self.style.row);
                pieces.push((text, new, None));
            }
            for (c, cell) in row.cells.iter().enumerate() {
                if c > 0 {
                    let (text, new) = new_or(&row.seps[c - 1], self.style.col);
                    pieces.push((text, new, None));
                }
                pieces.push((cell.clone(), false, Some((r, c))));
            }
        }
        if self.drops_last_row() {
            pieces.push((self.style.row.clone(), true, None));
        }
        pieces.push((edge(&self.tail), false, None));
        let mut text = String::new();
        let mut cells: Vec<Vec<Range<usize>>> = self.rows.iter().map(|_| Vec::new()).collect();
        for i in 0..pieces.len() {
            let (piece, new, cell) = &pieces[i];
            let mut piece = piece.as_str();
            if *new {
                if text.chars().last().is_none_or(char::is_whitespace) {
                    piece = piece.trim_start_matches(' ');
                }
                let next = pieces[i + 1..].iter().find_map(|p| p.0.chars().next());
                match next {
                    Some('\n') => piece = piece.trim_end_matches([' ', '\n']),
                    Some(c) if c.is_whitespace() => piece = piece.trim_end_matches(' '),
                    // Before `\end`: a row break ends there, a `&` keeps
                    // its space (`b & \end`).
                    None if piece.trim() != "&" => piece = piece.trim_end_matches(' '),
                    _ => {}
                }
            }
            let start = text.len();
            text.push_str(piece);
            if let Some((r, _)) = cell {
                cells[*r].push(start..text.len());
            }
        }
        Written { text, cells }
    }

    /// Whether KaTeX would drop the last row (one empty cell after a
    /// `\\`): a `\\` after it keeps it, as `\placeholder{}` did.
    fn drops_last_row(&self) -> bool {
        self.rows.len() > 1
            && self.width() == 1
            && self.row_empty(self.rows.len() - 1)
            && !self.tail.contains(r"\\")
    }

    /// Cell `(r, c)`'s index among the grid's cells, row by row.
    pub fn index(&self, r: usize, c: usize) -> usize {
        self.rows[..r]
            .iter()
            .map(|row| row.cells.len())
            .sum::<usize>()
            + c
    }
}
