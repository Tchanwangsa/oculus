use crate::types::Mode;
use crate::{
    KatexContext, ParseError, Settings,
    macros::MacroContextInterface as _,
    parser::{
        Parser,
        parse_node::{ParseNode, ParseNodeTag},
    },
    types::{ParseErrorKind, SourceLocation, Token},
};

/// Parses an expression using a Parser, then returns the parsed result.
pub fn parse_tree(
    ctx: &KatexContext,
    expr: &str,
    settings: &Settings,
) -> Result<Vec<ParseNode>, ParseError> {
    let mut parser = Parser::new(expr, settings, ctx);
    // Blank out any \df@tag to avoid spurious "Duplicate \tag" errors
    parser.gullet.macros_mut().purge("\\df@tag");
    let tree = parser.parse()?;
    // Prevent a color definition from persisting between calls to
    // katex.render().
    parser.gullet.macros_mut().purge("\\current@color");
    parser.gullet.macros_mut().purge("\\color");
    // If the input used \tag, it will set the \df@tag macro to the tag.
    // In this case, we separately parse the tag and wrap the tree.
    if parser.gullet.macros().get("\\df@tag").is_some() {
        if !settings.display_mode {
            return Err(ParseError::new(ParseErrorKind::TagNotAllowedInInlineMode));
        }
        let mut tag = parser.subparse(vec![Token::new("\\df@tag", None)])?;
        // With source mapping on, the wrapper is the whole formula.
        let loc = settings.source_map.then(|| {
            let input = parser.gullet.input_arc();
            let len = input.len();
            SourceLocation::new(input, 0, len)
        });
        if settings.source_map {
            for node in &mut tag {
                node.cover_children();
            }
        }
        let tree = vec![ParseNode::Tag(ParseNodeTag {
            mode: Mode::Text,
            loc,
            body: tree,
            tag,
        })];

        return Ok(tree);
    }

    Ok(tree)
}
