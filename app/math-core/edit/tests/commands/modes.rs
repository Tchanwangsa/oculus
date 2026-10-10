//! Text mode, command mode, and the queries the view asks.

use oculus_math_edit::{Command, Mode};

use super::harness::{Case, check_all, field};

/// doc 84-87.
#[test]
fn text_mode_types_every_character_literally() {
    let cases: &[Case] = &[
        (r"\text{|}", &["t:a", "space", "t:b"], r"\text{a b|}"),
        (
            r"\text{|}",
            &[r"t:{}$%#&_^~\"],
            r"\text{\{\}\$\%\#\&\_\textasciicircum{}\textasciitilde{}\textbackslash{}|}",
        ),
        // An IME's syllable goes in whole.
        (r"\text{|}", &["ime:สวัสดี"], r"\text{สวัสดี|}"),
        (r"\text{a|}", &["t:/^"], r"\text{a/\textasciicircum{}|}"),
        (r"\text{\LaTeX|}", &["t:x"], r"\text{\LaTeX x|}"),
    ];
    check_all(cases, false);
}

#[test]
fn command_mode_commits_with_the_arguments_as_empty_slots() {
    let cases: &[Case] = &[
        ("|", &[r"t:\alp"], r"|  ⟨\alp⟩"),
        ("|", &[r"t:\alpha", "space"], r"\alpha|"),
        ("x|", &[r"t:\alpha", "enter"], r"x\alpha|"),
        ("x|", &[r"t:\alpha", "tab"], r"x\alpha|"),
        ("|", &[r"t:\frac", "space"], r"\frac{|}{}"),
        ("|", &[r"t:\sqrt", "space", "t:2"], r"\sqrt{2|}"),
        ("‹x›", &[r"t:\sqrt", "space"], r"\sqrt{x}|"),
        ("‹x›", &[r"t:\hat", "space"], r"\hat{x}|"),
        // A non-letter commits, then is typed.
        ("|", &[r"t:\alpha+"], r"\alpha+|"),
        ("|", &[r"t:\alpha\beta", "space"], r"\alpha\beta|"),
        ("|x", &[r"t:\alpha", "space"], r"\alpha |x"),
        // Unknown: not inserted, still pending.
        ("|", &[r"t:\zzz", "space"], r"|  ⟨\zzz⟩"),
        ("|", &[r"t:\al", "bs"], r"|  ⟨\a⟩"),
        ("|", &[r"t:\al", "bs", "bs", "bs"], "|"),
        ("a|", &[r"t:\al", "esc"], "a|"),
        // Another key drops the command, then acts.
        ("a|", &[r"t:\al", "left"], "|a"),
        // Control symbols.
        ("a|", &[r"t:\,"], r"a\,|"),
        ("a|", &[r"t:\", "space"], r"a\ |"),
        ("a|", &[r"t:\{"], r"a\{|"),
    ];
    check_all(cases, false);
}

/// doc 84-87.
#[test]
fn committing_a_text_command_starts_text_mode() {
    let cases: &[Case] = &[
        (
            "|",
            &[r"t:\text", "space", "t:a", "space", "t:b"],
            r"\text{a b|}",
        ),
        ("|", &[r"t:\textbf", "space", "t:a"], r"\textbf{a|}"),
        ("|", &[r"t:\textit", "space", "t:a"], r"\textit{a|}"),
        // KaTeX has no `\mbox`: it stays pending as unknown.
        ("|", &[r"t:\mbox", "space"], r"|  ⟨\mbox⟩"),
        (
            "|",
            &[r"t:\textnormal", "space", "t:a", "tab", "t:x"],
            r"\textnormal{a}x|",
        ),
    ];
    check_all(cases, false);
}

#[test]
fn the_mode_and_whether_space_is_free() {
    let maths = field("a|", false);
    assert_eq!(maths.mode(), Mode::Math);
    assert!(maths.space_free());
    let text = field(r"\text{a|}", false);
    assert_eq!(text.mode(), Mode::Text);
    assert!(!text.space_free());
    // Beside a text run is maths: the run's end is a stop of its own.
    assert!(field(r"\text{a}|", false).space_free());
    let pending = maths.run(&Command::Insert("\\".to_owned())).field;
    assert_eq!(pending.mode(), Mode::Command);
    assert_eq!(pending.pending(), Some(""));
    assert!(!pending.space_free());
}
