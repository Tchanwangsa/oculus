//! Order-keeping JSON: an object read as its entries in file order, each
//! value kept as written, so a rewrite of the student's settings file
//! changes only the entries it means to (`cleanup.rs`).

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use serde_json::Value;

/// A JSON object read as its entries, in file order, values untouched.
pub(super) struct Ordered(pub(super) Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Box<RawValue>>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(V)
    }
}

pub(super) enum Node {
    Raw(Box<RawValue>),
    Value(Value),
    Map(Vec<(String, Node)>),
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Node::Raw(r) => r.serialize(s),
            Node::Value(v) => v.serialize(s),
            Node::Map(entries) => {
                let mut m = s.serialize_map(Some(entries.len()))?;
                for (k, v) in entries {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }
    }
}
