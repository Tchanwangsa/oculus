//! `Style` -> `taffy::Style`. Every element is a flex container; the
//! direction defaults to `row`, as in CSS.

use crate::scene::{Align, Dim, FlexDirection, FlexWrap, Justify, Position, Sides, Style};
use taffy::prelude::{auto, length, percent};
use taffy::{
    AlignItems, Dimension, Display, JustifyContent, LengthPercentage, LengthPercentageAuto, Rect,
    Size,
};

fn dim(d: Option<Dim>) -> Dimension {
    match d.unwrap_or_default() {
        Dim::Auto => auto(),
        Dim::Px(v) => length(v),
        Dim::Pct(p) => percent(p),
    }
}

fn inset(d: Option<Dim>) -> LengthPercentageAuto {
    match d.unwrap_or_default() {
        Dim::Auto => auto(),
        Dim::Px(v) => length(v),
        Dim::Pct(p) => percent(p),
    }
}

fn sides<T>(s: Option<Sides>, f: impl Fn(f32) -> T) -> Rect<T> {
    let s = s.unwrap_or_default();
    Rect {
        top: f(s.top),
        right: f(s.right),
        bottom: f(s.bottom),
        left: f(s.left),
    }
}

fn align(a: Align) -> AlignItems {
    match a {
        Align::Start => AlignItems::START,
        Align::End => AlignItems::END,
        Align::FlexStart => AlignItems::FLEX_START,
        Align::FlexEnd => AlignItems::FLEX_END,
        Align::Center => AlignItems::CENTER,
        Align::Stretch => AlignItems::STRETCH,
        Align::Baseline => AlignItems::BASELINE,
    }
}

fn justify(j: Justify) -> JustifyContent {
    match j {
        Justify::Start => JustifyContent::START,
        Justify::End => JustifyContent::END,
        Justify::FlexStart => JustifyContent::FLEX_START,
        Justify::FlexEnd => JustifyContent::FLEX_END,
        Justify::Center => JustifyContent::CENTER,
        Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        Justify::SpaceAround => JustifyContent::SPACE_AROUND,
        Justify::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    }
}

pub fn to_taffy(s: &Style) -> taffy::Style {
    let gap = length(s.gap.unwrap_or(0.0).max(0.0));
    let border = s.border.as_ref().map_or(0.0, |b| b.width.max(0.0));
    let positive = |v: f32| -> LengthPercentage { length(v.max(0.0)) };
    taffy::Style {
        display: Display::Flex,
        flex_direction: match s.flex_direction.unwrap_or_default() {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::Column => taffy::FlexDirection::Column,
            FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        flex_wrap: match s.flex_wrap {
            Some(FlexWrap::Wrap) => taffy::FlexWrap::Wrap,
            _ => taffy::FlexWrap::NoWrap,
        },
        flex_grow: s.flex_grow.unwrap_or(0.0).max(0.0),
        flex_shrink: s.flex_shrink.unwrap_or(1.0).max(0.0),
        size: Size {
            width: dim(s.width),
            height: dim(s.height),
        },
        min_size: Size {
            width: inset(s.min_width),
            height: inset(s.min_height),
        },
        max_size: Size {
            width: inset(s.max_width),
            height: inset(s.max_height),
        },
        padding: sides(s.padding, positive),
        margin: sides(s.margin, |v| -> LengthPercentageAuto { length(v) }),
        border: Rect {
            top: length(border),
            right: length(border),
            bottom: length(border),
            left: length(border),
        },
        gap: Size {
            width: gap,
            height: gap,
        },
        align_items: s.align_items.map(align),
        align_self: s.align_self.map(align),
        justify_content: s.justify_content.map(justify),
        position: match s.position {
            Some(Position::Absolute) => taffy::Position::Absolute,
            _ => taffy::Position::Relative,
        },
        inset: Rect {
            top: inset(s.top),
            right: inset(s.right),
            bottom: inset(s.bottom),
            left: inset(s.left),
        },
        ..taffy::Style::default()
    }
}
