//! Checks a scene's props against a component's schema, fills defaults and
//! resolves token names to their theme values. The result is what the
//! component is called with.

use super::{PropKind, PropSpec, PropsSchema};
use crate::error::StudioError;
use crate::scene::describe;
use crate::suggest::did_you_mean;
use crate::theme::Theme;
use serde_json::{Map, Value};

pub fn validate_props(
    component: &str,
    schema: &PropsSchema,
    props: &Map<String, Value>,
    theme: &Theme,
) -> Result<Map<String, Value>, StudioError> {
    let err = |path: &str, expected: String, got: String| StudioError::Props {
        component: component.to_owned(),
        path: path.to_owned(),
        expected,
        got,
    };
    let names = || schema.iter().map(|(n, _)| n.as_str());
    for (k, v) in props {
        if !schema.iter().any(|(n, _)| n == k) {
            let list = names().collect::<Vec<_>>().join(", ");
            return Err(err(
                k,
                format!(
                    "no such prop{}; {component} takes: {}",
                    did_you_mean(k, names()),
                    if list.is_empty() { "nothing" } else { &list }
                ),
                describe(v),
            ));
        }
    }
    let mut out = Map::new();
    for (name, spec) in schema {
        match props.get(name) {
            Some(v) => {
                out.insert(name.clone(), check(spec, v, name, theme, &err)?);
            }
            None => match &spec.default {
                Some(d) => {
                    let checked = check(spec, d, name, theme, &err).map_err(|e| match e {
                        StudioError::Props {
                            component,
                            path,
                            expected,
                            got,
                        } => StudioError::Props {
                            component,
                            path: format!("{path} (its default)"),
                            expected,
                            got,
                        },
                        other => other,
                    })?;
                    out.insert(name.clone(), checked);
                }
                None if spec.optional => {}
                None => {
                    return Err(err(
                        name,
                        format!("{} (required)", spec.kind.describe()),
                        "nothing (missing)".to_owned(),
                    ));
                }
            },
        }
    }
    Ok(out)
}

fn check(
    spec: &PropSpec,
    v: &Value,
    path: &str,
    theme: &Theme,
    err: &dyn Fn(&str, String, String) -> StudioError,
) -> Result<Value, StudioError> {
    let wrong = || err(path, spec.kind.describe(), describe(v));
    match &spec.kind {
        PropKind::Str | PropKind::Latex => v.is_string().then(|| v.clone()).ok_or_else(wrong),
        PropKind::Num => v.is_number().then(|| v.clone()).ok_or_else(wrong),
        PropKind::Bool => v.is_boolean().then(|| v.clone()).ok_or_else(wrong),
        PropKind::OneOf(values) => values.contains(v).then(|| v.clone()).ok_or_else(wrong),
        PropKind::List(item) => {
            let items = v.as_array().ok_or_else(wrong)?;
            items
                .iter()
                .enumerate()
                .map(|(i, x)| check(item, x, &format!("{path}[{i}]"), theme, err))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        PropKind::Token(kind) => {
            let Some(section) = theme.section(kind) else {
                return Err(err(
                    path,
                    format!(
                        "token(\"{kind}\") to name a theme section, but the theme has no `{kind}`"
                    ),
                    describe(v),
                ));
            };
            let name = v.as_str().ok_or_else(wrong)?;
            section.get(name).cloned().ok_or_else(|| {
                let names = section.keys().map(String::as_str);
                err(
                    path,
                    format!(
                        "a theme.{kind} token{}; tokens: {}",
                        did_you_mean(name, names.clone()),
                        names.collect::<Vec<_>>().join(", ")
                    ),
                    describe(v),
                )
            })
        }
    }
}
