//! Matrices typed as in MATLAB: Space, `;`, Backspace and the closing
//! Matrices typed as in MATLAB: Space, `;`, Backspace and the closing
//! bracket in a grid, `&` in an array.

use oculus_math_edit::{Command, Field};

use super::harness::{Case, check_all, field, press};

/// Matching or ghost right delimiters draw a matrix; mismatched ones don't.
#[test]
fn bracket_groups_whose_brackets_draw_a_matrix() {
    let cases: &[Case] = &[
        ("(a|", &["space"], r"\begin{pmatrix}a & |\end{pmatrix}"),
        ("(a|)", &["space"], r"\begin{pmatrix}a & |\end{pmatrix}"),
        (
            r"\lbrack a|\rbrack",
            &["space"],
            r"\begin{bmatrix}a & |\end{bmatrix}",
        ),
        ("[a|]", &["space"], r"\begin{bmatrix}a & |\end{bmatrix}"),
        (r"\{a|\}", &["space"], r"\begin{Bmatrix}a & |\end{Bmatrix}"),
        (
            r"\lbrace a|",
            &["space"],
            r"\begin{Bmatrix}a & |\end{Bmatrix}",
        ),
        (
            r"\vert a|\vert",
            &["space"],
            r"\begin{vmatrix}a & |\end{vmatrix}",
        ),
        (
            r"\lvert a|\rvert",
            &["space"],
            r"\begin{vmatrix}a & |\end{vmatrix}",
        ),
        (
            r"\Vert a|\Vert",
            &["space"],
            r"\begin{Vmatrix}a & |\end{Vmatrix}",
        ),
        // Mismatched brackets are no grid: an interval stays one.
        ("[a|)", &["space"], "[a|)"),
        (r"\langle a|\rangle", &["space"], r"\langle a|\rangle"),
        // A closed pair before the caret is not around it.
        ("(a)|", &["space"], "(a)|"),
        // The innermost open bracket is the grid.
        ("[f(x|", &["space"], r"[f\begin{pmatrix}x & |\end{pmatrix}"),
    ];
    check_all(cases, false);
}

/// A `\left…\right` pair is a bracket group too, and becomes plain
/// environment brackets.
#[test]
fn left_right_groups() {
    let cases: &[Case] = &[
        (
            r"\left(a|\right)",
            &["space"],
            r"\begin{pmatrix}a & |\end{pmatrix}",
        ),
        (
            r"\left[a|\right]^T",
            &["space"],
            r"\begin{bmatrix}a & |\end{bmatrix}^T",
        ),
        (r"\left[a|\right)", &["space"], r"\left[a|\right)"),
        (
            r"\left( a|\right)",
            &["t:;"],
            r"\begin{pmatrix} a \\ |\\\end{pmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// A one-cell grid is written as its bracket group, a ghost unless closed.
/// The group comes back unclosed, so the closer the user types next is the
/// only one.
#[test]
fn one_cell_left_is_a_bracket_group_again() {
    let cases: &[Case] = &[
        (r"\begin{pmatrix}a & |\end{pmatrix}", &["bs"], "(a|"),
        (r"\begin{pmatrix}a & |\end{pmatrix}", &["bs", "t:)"], "(a)|"),
        (r"\begin{bmatrix}x|\end{bmatrix}", &["t:]"], "[x]|"),
        // No brackets to fall back to: stays a matrix.
        (
            r"\begin{matrix}x & |\end{matrix}",
            &["bs"],
            r"\begin{matrix}x|\end{matrix}",
        ),
        // Scripts on the matrix stay on its group, which is then closed.
        (r"\begin{bmatrix}a & |\end{bmatrix}^T", &["bs"], "[a|]^T"),
        (r"\begin{Bmatrix}a & |\end{Bmatrix}", &["bs"], r"\{a|"),
        (r"\begin{Vmatrix}a & |\end{Vmatrix}", &["bs"], r"\|a|"),
        // A `\left…\right` group comes back as plain brackets.
        (r"\left(a|\right)", &["space", "bs"], "(a|"),
    ];
    check_all(cases, false);
}

/// Empty cells are stored empty, not as placeholders.
#[test]
fn empty_cells_are_stored_empty() {
    let cases: &[Case] = &[(
        "[|",
        &["t:a", "space", "t:b", "t:;"],
        r"\begin{bmatrix}a & b \\ |& \end{bmatrix}",
    )];
    check_all(cases, false);
}

/// A control word before a letter keeps a space.
#[test]
fn a_control_word_keeps_its_space() {
    let cases: &[Case] = &[
        (
            r"\begin{pmatrix}\alpha & |\end{pmatrix}x",
            &["bs"],
            r"(\alpha |x",
        ),
        (
            r"\begin{bmatrix}\alpha & \le|\end{bmatrix}",
            &["space"],
            r"[\alpha\le|",
        ),
        (
            r"[\alpha|",
            &["space"],
            r"\begin{bmatrix}\alpha & |\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// Space after a term in a bracket group starts a second cell.
#[test]
fn space_after_a_term_starts_a_cell() {
    check_all(
        &[("(a|", &["space"], r"\begin{pmatrix}a & |\end{pmatrix}")],
        false,
    );
}

/// Space leaves an empty cell, an operator's end or a cell's start to the
/// toolbox, and likewise after `\sin`-like and big operators.
#[test]
fn space_is_the_views_in_an_empty_cell_after_an_operator_or_at_a_start() {
    let cases: &[Case] = &[
        ("(|", &["space"], "(|"),
        ("[a+|", &["space"], "[a+|"),
        ("[|a", &["space"], "[|a"),
        ("[a=|", &["space"], "[a=|"),
        ("[a,|", &["space"], "[a,|"),
        ("[(|", &["space"], "[(|"),
        (r"[\sin|", &["space"], r"[\sin|"),
        (r"[\sum_{i}|", &["space"], r"[\sum_{i}|"),
        (r"[\operatorname{f}|", &["space"], r"[\operatorname{f}|"),
        (
            r"\begin{bmatrix}a & |\end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a & |\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// `(a + b)` with habitual spaces stays one cell, back to plain brackets.
#[test]
fn habitual_spaces_around_an_operator_keep_one_cell() {
    let cases: &[Case] = &[
        ("(|", &["t:a", "space", "t:+", "space", "t:b"], "(a+b|"),
        (
            "(|",
            &["t:a", "space", "t:+", "space", "t:b", "t:)"],
            "(a+b)|",
        ),
        ("(|", &["t:a", "space", "t:=", "space", "t:b"], "(a=b|"),
    ];
    check_all(cases, false);
}

/// `[1 -1]` keeps two cells.
#[test]
fn a_leading_minus_starts_a_cell() {
    check_all(
        &[(
            "[|",
            &["t:1", "space", "t:-1", "t:]"],
            r"\begin{bmatrix}1 & -1\end{bmatrix}|",
        )],
        false,
    );
}

/// An operator merged in a row of a taller grid shifts the row's later
/// cells left.
#[test]
fn an_operator_merged_in_a_taller_grid_shifts_the_row_left() {
    check_all(
        &[(
            r"\begin{bmatrix}a & b & c \\ d & =| & e\end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a & b & c \\ d=| & e & \end{bmatrix}",
        )],
        false,
    );
}

/// An operator alone in a row's first cell is not merged.
#[test]
fn an_operator_alone_in_a_first_cell_stays() {
    let cases: &[Case] = &[
        ("[+|", &["space"], "[+|"),
        (
            r"\begin{bmatrix}a \\ =|\end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a \\ =|\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// Space mid-cell splits off what follows the caret into a new column.
#[test]
fn space_mid_cell_splits_into_a_new_column() {
    check_all(
        &[(
            r"\begin{bmatrix}a|b & c \\ d & e\end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a & |b & c \\ d & & e\end{bmatrix}",
        )],
        false,
    );
}

/// Space at a cell's end with an empty next cell only moves into it.
#[test]
fn space_at_a_cells_end_moves_into_an_empty_next_cell() {
    check_all(
        &[(
            r"\begin{bmatrix}a & b \\ c| & \end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a & b \\ c & |\end{bmatrix}",
        )],
        false,
    );
}

/// Space stops at ten columns.
#[test]
fn space_stops_at_ten_columns() {
    let ten = r"\begin{bmatrix}1&2&3&4&5&6&7&8&9&10|\end{bmatrix}";
    check_all(&[(ten, &["space"], ten)], false);
}

/// `[a b; c d e f g]` pads the first row as the second grows.
#[test]
fn rows_are_padded_as_a_row_grows() {
    let keys = &[
        "t:a", "space", "t:b", "t:;", "t:c", "space", "t:d", "space", "t:e", "space", "t:f",
        "space", "t:g", "t:]",
    ];
    check_all(
        &[(
            "[|",
            keys,
            r"\begin{bmatrix}a & b & & & \\ c & d & e & f & g\end{bmatrix}|",
        )],
        false,
    );
}

/// A ragged grid read from LaTeX is padded before a split: the move into a
/// cell the row lacks pads it first.
#[test]
fn a_ragged_grid_is_padded_before_an_edit() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}a & b| \\ c & d & e & f & g\end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a & b & |& & \\ c & d & e & f & g\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a & b \\ c & d & e & f & g|\end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a & b & & & & \\ c & d & e & f & g & |\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// `;` in a bracket group adds a row. The group becomes the matrix in one
/// step.
#[test]
fn semicolon_in_a_bracket_group_adds_a_row() {
    let cases: &[Case] = &[
        ("(x|", &["t:;"], r"\begin{pmatrix}x \\ |\\\end{pmatrix}"),
        // A column vector: the empty last row is kept by a `\\` after it,
        // and typing into it fills it.
        (
            "(|",
            &["t:1", "t:;", "t:2", "t:;", "t:3", "t:)"],
            r"\begin{pmatrix}1 \\ 2 \\ 3\end{pmatrix}|",
        ),
        (
            "f(x|",
            &["t:;", "t:y"],
            r"f\begin{pmatrix}x \\ y| \\\end{pmatrix}",
        ),
        // Typed as one cell (a space in the source is no Space key).
        (
            "[a b|]",
            &["t:;"],
            r"\begin{bmatrix}a b \\ |\\\end{bmatrix}",
        ),
        // A group that is a bare script takes braces as a matrix.
        ("x^(|", &["t:;"], r"x^{\begin{pmatrix}\\ |\\\end{pmatrix}}"),
        // Backspace in the new row takes it and the `\\` that kept it.
        (
            r"\begin{bmatrix}a \\ b|\end{bmatrix}",
            &["t:;", "bs"],
            r"\begin{bmatrix}a \\ b|\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// `;` in a middle row inserts an empty row after it, or moves into an
/// empty one.
#[test]
fn semicolon_in_a_middle_row() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}a & b| \\ c & d \\ e & f\end{bmatrix}",
            &["t:;"],
            r"\begin{bmatrix}a & b \\ |& \\ c & d \\ e & f\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a & b| \\ & \\ e & f\end{bmatrix}",
            &["t:;"],
            r"\begin{bmatrix}a & b \\ |& \\ e & f\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// New rows take the widest row's width.
#[test]
fn new_rows_take_the_widest_rows_width() {
    check_all(
        &[(
            r"\begin{bmatrix}a \\ b & c & d|\end{bmatrix}",
            &["t:;"],
            r"\begin{bmatrix}a & & \\ b & c & d \\ |& & \end{bmatrix}",
        )],
        false,
    );
}

/// Backspace in an empty column removes it; at one cell left the grid is a
/// bracket group again.
#[test]
fn backspace_in_an_empty_column_removes_it() {
    check_all(
        &[(r"\begin{pmatrix}a & |\end{pmatrix}", &["bs"], "(a|")],
        false,
    );
}

/// Backspace removes the column when it is all empty, else the row.
#[test]
fn backspace_removes_an_empty_column_else_an_empty_row() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}a & & b \\ c & |& d\end{bmatrix}",
            &["bs"],
            r"\begin{bmatrix}a & b \\ c| & d\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a & b \\ & |\end{bmatrix}",
            &["bs"],
            r"\begin{bmatrix}a & b|\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// A first column removed puts the caret at the previous row's end.
#[test]
fn a_first_column_removed_puts_the_caret_at_the_previous_rows_end() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}& a \\ |& b\end{bmatrix}",
            &["bs"],
            r"\begin{bmatrix}a| \\ b\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}|& a \\ & b\end{bmatrix}",
            &["bs"],
            r"\begin{bmatrix}|a \\ b\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// Backspace in an empty one-column row collapses a column vector to its
/// group.
#[test]
fn an_empty_row_collapses_a_column_vector_to_its_group() {
    let cases: &[Case] = &[
        (r"\begin{pmatrix}x \\ |\\\end{pmatrix}", &["bs"], "(x|"),
        ("(x|", &["t:;", "bs"], "(x|"),
    ];
    check_all(cases, false);
}

/// Backspace in an empty cell beside full ones steps back a cell, row-
/// major.
#[test]
fn backspace_in_an_empty_cell_beside_full_ones_steps_back() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}a & b \\ |& d\end{bmatrix}",
            &["bs"],
            r"\begin{bmatrix}a & b| \\ & d\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a & |\\ c & d\end{bmatrix}",
            &["bs"],
            r"\begin{bmatrix}a| & \\ c & d\end{bmatrix}",
        ),
        // The first cell: Backspace's own rule selects the matrix.
        (
            r"\begin{bmatrix}|& b \\ c & d\end{bmatrix}",
            &["bs"],
            r"›\begin{bmatrix}& b \\ c & d\end{bmatrix}‹",
        ),
    ];
    check_all(cases, false);
}

/// Backspace in a cell with content, or in a one-cell grid, follows
/// Backspace's own rules.
#[test]
fn backspace_leaves_a_full_cell_and_a_one_cell_grid_alone() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}a & |b\end{bmatrix}",
            &["bs"],
            r"\begin{bmatrix}a| & b\end{bmatrix}",
        ),
        ("(|", &["bs"], "|"),
        (r"\begin{bmatrix}|\end{bmatrix}", &["bs"], "|"),
    ];
    check_all(cases, false);
}

/// The closing bracket drops trailing empty rows and columns.
#[test]
fn the_closing_bracket_drops_trailing_empty_rows_and_columns() {
    check_all(
        &[(
            r"\begin{bmatrix}a & b & \\ c & & \\ & |& \end{bmatrix}",
            &["t:]"],
            r"\begin{bmatrix}a & b \\ c & \end{bmatrix}|",
        )],
        false,
    );
}

/// A single cell left closes as a bracket group.
#[test]
fn a_single_cell_left_closes_as_a_bracket_group() {
    check_all(
        &[(r"\begin{pmatrix}a & \\ & |\end{pmatrix}", &["t:)"], "(a)|")],
        false,
    );
}

/// Each matrix's own closer, the caret after its scripts; a bracket group's
/// closer is typed as itself; `}` closes a `Bmatrix` only from the cell
/// itself, not from a `{…}` inside it. Matrices with no closing key
/// (`Vmatrix`, `matrix`) type the character.
#[test]
fn closers_per_environment() {
    let cases: &[Case] = &[
        (
            r"\begin{pmatrix}a & b|\end{pmatrix}",
            &["t:)"],
            r"\begin{pmatrix}a & b\end{pmatrix}|",
        ),
        (
            r"\begin{bmatrix}a & b|\end{bmatrix}^T",
            &["t:]"],
            r"\begin{bmatrix}a & b\end{bmatrix}^T¦",
        ),
        (
            r"\begin{Bmatrix}a & b|\end{Bmatrix}",
            &["t:}"],
            r"\begin{Bmatrix}a & b\end{Bmatrix}|",
        ),
        (
            r"\begin{Bmatrix}a & {b|}\end{Bmatrix}",
            &["t:}"],
            r"\begin{Bmatrix}a & {b}|\end{Bmatrix}",
        ),
        (
            r"\begin{vmatrix}a & b|\end{vmatrix}",
            &["t:|"],
            r"\begin{vmatrix}a & b\end{vmatrix}|",
        ),
        (
            r"\begin{bmatrix}a & b|\end{bmatrix}",
            &["t:)"],
            r"\begin{bmatrix}a & b)|\end{bmatrix}",
        ),
        (
            r"\begin{matrix}a & b|\end{matrix}",
            &["t:]"],
            r"\begin{matrix}a & b]|\end{matrix}",
        ),
        // One cell left: a closed group of plain bars.
        (r"\vert a|\vert", &["space", "t:|"], "|a||"),
        // In a group inside a cell the closer closes the group.
        (
            r"\begin{pmatrix}f(x|\end{pmatrix}",
            &["t:)"],
            r"\begin{pmatrix}f(x)|\end{pmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// `[a b]^T` keeps its scripts on the matrix.
#[test]
fn scripts_on_the_closer_stay_on_the_matrix() {
    let cases: &[Case] = &[
        (
            "[a|]^T",
            &["space", "t:b"],
            r"\begin{bmatrix}a & b|\end{bmatrix}^T",
        ),
        (
            "[a| b]^T",
            &["space"],
            r"\begin{bmatrix}a & |b\end{bmatrix}^T",
        ),
    ];
    check_all(cases, false);
}

/// `&` starts a new cell in any array cell (`aligned` too); elsewhere it
/// is `\&`.
#[test]
fn ampersand_in_and_out_of_arrays() {
    let cases: &[Case] = &[
        (
            r"\begin{aligned}a|\end{aligned}",
            &["t:&"],
            r"\begin{aligned}a&|\end{aligned}",
        ),
        (
            r"\begin{bmatrix}a|b\end{bmatrix}",
            &["t:&"],
            r"\begin{bmatrix}a&|b\end{bmatrix}",
        ),
        ("a|", &["t:&"], r"a\&|"),
        ("(a|", &["t:&"], r"(a\&|"),
        // Not directly in the cell: a raw `&` would not render.
        (
            r"\begin{aligned}\frac{a|}{b}\end{aligned}",
            &["t:&"],
            r"\begin{aligned}\frac{a\&|}{b}\end{aligned}",
        ),
    ];
    check_all(cases, false);
}

/// Typing into an empty spaced cell keeps the space before the next
/// separator, and Backspace takes it back with the character.
#[test]
fn spaced_empty_cells_stay_spaced() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}a & |& b\end{bmatrix}",
            &["t:c"],
            r"\begin{bmatrix}a & c| & b\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a & |& b\end{bmatrix}",
            &["t:c", "bs"],
            r"\begin{bmatrix}a & |& b\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a&|&b\end{bmatrix}",
            &["t:c"],
            r"\begin{bmatrix}a&c|&b\end{bmatrix}",
        ),
        // A control word's range takes the space after it; deleting it
        // still gives the cell's spacing back.
        (
            r"\begin{bmatrix}a & |& b\end{bmatrix}",
            &["t:~", "bs"],
            r"\begin{bmatrix}a & |& b\end{bmatrix}",
        ),
    ];
    check_all(cases, false);
    // A cell on a line of its own keeps its line.
    let block = "\\begin{Bmatrix}\n{a} & \\\\\n&\n|\\end{Bmatrix}";
    check_all(&[(block, &["t:~", "bs"], block)], true);
}

/// New separators follow the matrix's own: a tight `&`, its `\\`.
#[test]
fn new_separators_follow_the_matrixs_style() {
    let cases: &[Case] = &[
        (
            r"\begin{bmatrix}a&b|\\c&d\end{bmatrix}",
            &["space"],
            r"\begin{bmatrix}a&b&|\\c&d&\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a&b|\end{bmatrix}",
            &["t:;"],
            r"\begin{bmatrix}a&b\\|&\end{bmatrix}",
        ),
        (
            "\\begin{bmatrix}a & b|\\\\[1em] c & d\\end{bmatrix}",
            &["t:;"],
            "\\begin{bmatrix}a & b \\\\ |& \\\\[1em] c & d\\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}a & b \\c & d|\end{bmatrix}",
            &["t:;"],
            r"\begin{bmatrix}a & b \\c & d \\ |& \end{bmatrix}",
        ),
    ];
    check_all(cases, false);
}

/// A matrix that is a whole display block goes one row per line; inside a
/// larger formula it stays on one line.
#[test]
fn rows_in_a_display_block_go_one_per_line() {
    let cases: &[Case] = &[
        (
            "[a|",
            &["space", "t:b", "t:;"],
            "\\begin{bmatrix}\na & b \\\\\n|&\n\\end{bmatrix}",
        ),
        (
            "\\begin{bmatrix}\na & b \\\\\nc & d|\n\\end{bmatrix}",
            &["t:;", "t:e"],
            "\\begin{bmatrix}\na & b \\\\\nc & d \\\\\ne| &\n\\end{bmatrix}",
        ),
        (
            "x = [a|",
            &["space", "t:b", "t:;"],
            r"x = \begin{bmatrix}a & b \\ |& \end{bmatrix}",
        ),
        (
            "(1|",
            &["t:;", "t:2", "t:)"],
            "\\begin{pmatrix}\n1 \\\\\n2\n\\end{pmatrix}|",
        ),
        // The last cell types on its `&`'s line, not before `\end`.
        (
            "[a|",
            &["space", "t:b", "t:;", "t:c", "space", "t:d", "t:]"],
            "\\begin{bmatrix}\na & b \\\\\nc & d\n\\end{bmatrix}|",
        ),
        (
            "[a|",
            &["space", "t:b", "t:;", "t:c", "space", "t:d", "bs"],
            "\\begin{bmatrix}\na & b \\\\\nc &\n|\\end{bmatrix}",
        ),
    ];
    check_all(cases, true);
}

/// Offsets are bytes: Thai in a `\text{}` cell (three bytes a character,
/// one UTF-16 unit) splits and joins on character boundaries.
#[test]
fn thai_in_a_text_cell() {
    let cases: &[Case] = &[
        (
            r"[\text{ไทย}|",
            &["space", r"tpl:\text{#0}", "ime:สวัสดี"],
            r"\begin{bmatrix}\text{ไทย} & \text{สวัสดี|}\end{bmatrix}",
        ),
        (
            r"\begin{bmatrix}\text{ไทย}| & \end{bmatrix}",
            &["right", "bs"],
            r"[\text{ไทย}|",
        ),
    ];
    check_all(cases, false);
}

/// `space_free`: Space is the view's unless it ends a cell or moves.
#[test]
fn space_free_where_space_is_no_grid_key() {
    let free = |marked: &str| field(marked, false).space_free();
    assert!(free("a|"));
    assert!(free("(|"));
    assert!(free("[a+|"));
    assert!(free(r"\begin{bmatrix}1&2&3&4&5&6&7&8&9&10|\end{bmatrix}"));
    assert!(free(r"\begin{aligned}a|&b\end{aligned}"));
    assert!(free(r"\begin{bmatrix}\frac{a|}{b}\end{bmatrix}"));
    assert!(!free("(a|"));
    assert!(!free(r"\begin{bmatrix}a| & \end{bmatrix}"));
    assert!(!free(r"\begin{bmatrix}a|b\end{bmatrix}"));
    // In a text run Space is a space; with a command pending it commits.
    assert!(!free(r"[\text{a|}"));
    let pending = field("(a|", false).run(&Command::Insert("\\".to_owned()));
    assert!(!pending.field.space_free());
}

/// Each grid edit is one change and an undo step of its own; a move is
/// no change.
#[test]
fn grid_edits_are_undo_steps_of_their_own() {
    let edit = field("(a|", false).run(&Command::Insert(" ".to_owned()));
    assert_eq!(edit.changes.len(), 1);
    assert!(edit.isolate);
    let typed = field("(a|", false).run(&Command::Insert("b".to_owned()));
    assert!(!typed.isolate);
    let moved: Field = field(r"\begin{bmatrix}a| & \end{bmatrix}", false)
        .run(&Command::Insert(" ".to_owned()))
        .field;
    assert_eq!(
        press(r"\begin{bmatrix}a| & \end{bmatrix}", false, &["space"]),
        super::harness::marked(&moved)
    );
}
