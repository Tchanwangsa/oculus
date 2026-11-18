use super::super::format::{js_number, serialize};

#[test]
fn numbers_print_as_javascript_prints_them() {
    for (value, expected) in [
        (0.0, "0"),
        (-0.0, "0"),
        (1.0, "1"),
        (-1.5, "-1.5"),
        (0.1, "0.1"),
        (0.1 + 0.2, "0.30000000000000004"),
        (100.0, "100"),
        (1e20, "100000000000000000000"),
        (1e21, "1e+21"),
        (1.5e21, "1.5e+21"),
        (1e-6, "0.000001"),
        (1.25e-6, "0.00000125"),
        (1e-7, "1e-7"),
        (-2.5e-8, "-2.5e-8"),
        (123456.789, "123456.789"),
        (f64::MAX, "1.7976931348623157e+308"),
        (5e-324, "5e-324"),
        (9007199254740993.0, "9007199254740992"),
    ] {
        assert_eq!(js_number(value), expected, "{value:e}");
    }
}

#[test]
fn serializer_matches_json_stringify_layout() {
    let value =
        serde_json::json!({"b": [], "a": {}, "n": [1, {"x": 2.0}], "s": "q\"\u{1}\u{2028}"});
    assert_eq!(
        serialize(&value).unwrap(),
        "{\n  \"b\": [],\n  \"a\": {},\n  \"n\": [\n    1,\n    {\n      \"x\": 2\n    }\n  ],\n  \"s\": \"q\\\"\\u0001\u{2028}\"\n}\n"
    );
}
