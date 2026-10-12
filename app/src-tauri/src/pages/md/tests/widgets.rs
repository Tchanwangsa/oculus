use super::md;

#[test]
fn accordion_pairs_become_details() {
    let out = md(r#"<div class="uom-ui-accordion">
<div class="uom-ui-accordion-trigger"><span class="uom-ui-accordion-title" style="color: #000b34;">Task Details (click for detail)</span></div>
<div class="uom-ui-accordion-panel"><p>Do <b>this</b>.</p><ul><li>a</li></ul></div>
<div class="uom-ui-accordion-trigger"><span class="uom-ui-accordion-title">Second</span></div>
<div class="uom-ui-accordion-panel"><p>Two</p></div>
</div>"#);
    assert_eq!(
        out,
        "<details>\n<summary>Task Details (click for detail)</summary>\n\nDo **this**.\n\n- a\n\n</details>\n\n\
         <details>\n<summary>Second</summary>\n\nTwo\n\n</details>"
    );
}

#[test]
fn mspace_accordions_and_bare_siblings_pair_too() {
    assert_eq!(
        md(
            r#"<h3>Steps</h3><div class="uom-mspace-accordion-trigger">Step 1: Read</div>
<div class="uom-mspace-accordion-panel"><p>Chapter 1</p></div>"#
        ),
        "### Steps\n\n<details>\n<summary>Step 1: Read</summary>\n\nChapter 1\n\n</details>"
    );
    // A trigger without a panel is its text; a panel without a trigger is its content.
    assert_eq!(
        md(
            r#"<div class="uom-ui-accordion-trigger">Lonely</div><p>x</p><div class="uom-ui-accordion-panel"><p>Orphan</p></div>"#
        ),
        "Lonely\n\nx\n\nOrphan"
    );
}

#[test]
fn reveal_answer_becomes_collapsed_details() {
    assert_eq!(
        md(r#"<p>What is 6 × 7?</p>
<p class="uom-ui-reveal uom-ui-button uom-primary" data-afterclick="disable">Reveal answer</p>
<div class="uom-ui-hidden-content uom-ui-emphasise"><p>42</p></div>
<p>Next</p>"#),
        "What is 6 × 7?\n\n<details>\n<summary>Reveal answer</summary>\n\n42\n\n</details>\n\nNext"
    );
    assert_eq!(
        md(r#"<p class="uom-ui-reveal uom-ui-button">Reveal answer</p><p>Not hidden</p>"#),
        "Reveal answer\n\nNot hidden"
    );
    assert_eq!(
        md(r#"<div class="uom-ui-hidden-content"><p>Shown</p></div>"#),
        "Shown"
    );
}

#[test]
fn tabs_pair_by_aria_controls_then_order() {
    let by_id = md(
        r#"<div class="uom-ui-tab"><div class="uom-ui-tablist" aria-label="tabcontent">
<button class="uom-ui-trigger" aria-controls="t2">Shape.java</button>
<button class="uom-ui-trigger" aria-controls="t1">Circle.java</button></div>
<div class="uom-ui-tab-panel" id="t1"><pre>class Circle {}</pre></div>
<div class="uom-ui-tab-panel" id="t2"><pre>class Shape {}</pre></div></div>"#,
    );
    assert_eq!(
        by_id,
        "<details>\n<summary>Shape.java</summary>\n\n```\nclass Shape {}\n```\n\n</details>\n\n\
         <details>\n<summary>Circle.java</summary>\n\n```\nclass Circle {}\n```\n\n</details>"
    );
    let by_order = md(r#"<div class="uom-ui-tab"><div class="uom-ui-tablist">
<a class="uom-ui-trigger">One</a><a class="uom-ui-trigger">Two</a><a class="uom-ui-trigger">Three</a></div>
<div class="uom-ui-tab-panel"><p>1</p></div><div class="uom-ui-tab-panel"><p>2</p></div></div>"#);
    assert_eq!(
        by_order,
        "<details>\n<summary>One</summary>\n\n1\n\n</details>\n\n\
         <details>\n<summary>Two</summary>\n\n2\n\n</details>\n\nThree"
    );
}

#[test]
fn checkboxes_become_one_tight_task_list() {
    assert_eq!(
        md(r#"<p class="uom-ui-checkbox uom-ui-clearstyle">Completed the end of module quiz</p>
<p class="uom-ui-checkbox uom-ui-clearstyle">Read <em>chapter</em> 2 ($5 book)</p>
<p>After</p><p class="uom-ui-checkbox">Again</p>"#),
        "- [ ] Completed the end of module quiz\n- [ ] Read *chapter* 2 (\\$5 book)\n\nAfter\n\n- [ ] Again"
    );
}

#[test]
fn tiles_flatten_to_blocks() {
    assert_eq!(
        md(r#"<table class="uom-ui-tiles uom-with-border"><tbody><tr>
<td style="height: 2047px;"><h3>Module 1</h3><p><a href="/courses/1/modules/2">Go</a></p></td>
<td><p>Module 2</p><table><tr><td>inner</td></tr></table></td></tr></tbody></table>"#),
        "### Module 1\n\n[Go](/courses/1/modules/2)\n\nModule 2\n\n| inner |\n| --- |"
    );
}

#[test]
fn callouts_become_labelled_blockquotes() {
    assert_eq!(
        md(r#"<div class="uom-ui-notice--info"><p>One</p><p>Two</p></div>"#),
        "> **Note**\n>\n> One\n>\n> Two"
    );
    assert_eq!(
        md(
            r#"<div class="alert alert-dismissible alert-warning" style="background-color: #fff3cd;"><p>Due <b>Friday</b></p></div>"#
        ),
        "> **Warning**\n>\n> Due **Friday**"
    );
    assert_eq!(
        md(r#"<div class="alert alert-success">Done</div>"#),
        "> **Tip**\n>\n> Done"
    );
    assert_eq!(
        md(
            r#"<div class="uom-ui-notice uom-ui-notice--danger"><ul><li>a</li><li>b</li></ul></div>"#
        ),
        "> **Warning**\n>\n> - a\n> - b"
    );
    assert_eq!(
        md(r#"<div class="uom-ui-notice"><p>Plain</p></div>"#),
        "> Plain"
    );
    assert_eq!(
        md(
            r#"<p class="uom-ui-emphasise">Key idea</p><p>a <span class="uom-ui-emphasise">b</span></p>"#
        ),
        "> Key idea\n\na b"
    );
}

#[test]
fn only_a_bordered_background_box_of_blocks_is_a_callout() {
    assert_eq!(
        md(
            r#"<div style="background: #fde2c6; border: 2px solid #f80; padding: 8px;"><p>Heads up</p><p>Twice</p></div>"#
        ),
        "> Heads up\n>\n> Twice"
    );
    for style in [
        "background: #fde2c6; border: none",
        "background-color: #eee; border: 0px solid",
        "border: 1px solid #000",
        "background-color: #f4f1fb; border-radius: 12px",
    ] {
        assert_eq!(
            md(&format!(r#"<div style="{style}"><p>x</p></div>"#)),
            "x",
            "{style}"
        );
    }
    assert_eq!(
        md(r#"<div style="border: 1px solid; background: #eee">just text</div>"#),
        "just text"
    );
}

#[test]
fn element_toggler_shows_its_target_as_details() {
    assert_eq!(
        md(
            r##"<p><a class="element_toggler" href="#" aria-controls="ans1" aria-expanded="false">Show answer</a></p>
<div id="ans1" style="display: none;"><p>It is 4.</p></div><p>Next</p>"##
        ),
        "<details>\n<summary>Show answer</summary>\n\nIt is 4.\n\n</details>\n\nNext"
    );
    // The target can come first; it still renders only at the toggler.
    assert_eq!(
        md(
            r#"<div id="h" style="display:none">Use x</div><span class="element_toggler" aria-controls="h">Hint</span>"#
        ),
        "<details>\n<summary>Hint</summary>\n\nUse x\n\n</details>"
    );
    // A toggler with no target is its text.
    assert_eq!(
        md(r#"<p><span class="element_toggler" aria-controls="nope">Hint</span></p>"#),
        "Hint"
    );
}

#[test]
fn code_theme_wrappers_become_fences() {
    assert_eq!(
        md("<div class=\"amber-display-codeblock amber-el amber-content\"><div class=\"amber-el amber-pre\"><pre>int x = 1;\nint y;</pre></div></div>"),
        "```\nint x = 1;\nint y;\n```"
    );
    assert_eq!(
        md(
            r#"<div class="amber-el amber-pre"><div class="syntax-highlight"><span>Shape[] s = </span><span class="syntax-keyword">new</span><span> Shape[2];</span></div><div>s[0] = null;</div></div>"#
        ),
        "```\nShape[] s = new Shape[2];\ns[0] = null;\n```"
    );
}

#[test]
fn widgets_in_table_cells_stay_on_one_line() {
    let out = md(r#"<table><tr><td>
<div class="uom-ui-accordion"><div class="uom-ui-accordion-trigger">T</div>
<div class="uom-ui-accordion-panel"><p>P</p></div></div></td>
<td><p class="uom-ui-reveal">Reveal answer</p>
<div class="uom-ui-hidden-content"><p>42</p></div></td>
<td><p class="uom-ui-checkbox">Done</p><div class="alert alert-info">Note this</div></td>
<td><span class="element_toggler" aria-controls="a">Hint</span></td></tr></table>
<div id="a" style="display:none"><p>Use x</p></div>"#);
    assert_eq!(
        out,
        "| T P | Reveal answer · 42 | Done · Note this | Hint Use x |\n| --- | --- | --- | --- |"
    );
}
