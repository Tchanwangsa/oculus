//! The request and response bodies of the multimodal endpoint.

use crate::embed::raster::RenderedPage;
use crate::embed::EmbedError;
use base64::Engine as _;
use serde_json::{json, Value};

/// The `inputs` array: one input per page, each the PNG `embed::raster` produced
/// as a data URI.
pub(super) fn image_inputs(pages: &[RenderedPage]) -> Value {
    Value::Array(
        pages
            .iter()
            .map(|page| {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&page.png);
                json!({
                    "content": [{
                        "type": "image_base64",
                        "image_base64": format!("data:image/png;base64,{encoded}"),
                    }],
                })
            })
            .collect(),
    )
}

/// Read the vectors out of a response, in input order — by each item's
/// `index`, not array order, so a reordered `data` cannot file one page's
/// vector under another.
pub(super) fn decode_response(
    payload: &Value,
    expected: usize,
) -> Result<Vec<Vec<f32>>, EmbedError> {
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or(EmbedError::Document {
            code: "invalid-response".into(),
        })?;
    if data.len() != expected {
        return Err(EmbedError::Document {
            code: "embedding-count-mismatch".into(),
        });
    }

    let mut slots: Vec<Option<Vec<f32>>> = vec![None; expected];
    for (position, item) in data.iter().enumerate() {
        let index = item
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or(position as u64) as usize;
        let slot = slots.get_mut(index).ok_or(EmbedError::Document {
            code: "embedding-index-out-of-range".into(),
        })?;
        if slot.is_some() {
            return Err(EmbedError::Document {
                code: "embedding-index-repeated".into(),
            });
        }
        *slot = Some(decode_embedding(item.get("embedding").ok_or(
            EmbedError::Document {
                code: "embedding-missing".into(),
            },
        )?)?);
    }
    slots
        .into_iter()
        .map(|slot| {
            slot.ok_or(EmbedError::Document {
                code: "embedding-missing".into(),
            })
        })
        .collect()
}

/// `output_encoding: "base64"` (f32 little-endian) -> `Vec<f32>`. A plain JSON
/// array is accepted too. Narrowing and normalising belong to
/// `embed::pack_vector` alone.
pub(super) fn decode_embedding(value: &Value) -> Result<Vec<f32>, EmbedError> {
    match value {
        Value::String(encoded) => {
            let raw = base64::engine::general_purpose::STANDARD
                .decode(encoded.trim())
                .map_err(|_| EmbedError::Document {
                    code: "embedding-not-base64".into(),
                })?;
            if raw.len() % 4 != 0 {
                return Err(EmbedError::Document {
                    code: "embedding-not-f32".into(),
                });
            }
            Ok(raw
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect())
        }
        Value::Array(numbers) => numbers
            .iter()
            .map(|number| {
                number
                    .as_f64()
                    .map(|value| value as f32)
                    .ok_or(EmbedError::Document {
                        code: "embedding-not-numeric".into(),
                    })
            })
            .collect(),
        _ => Err(EmbedError::Document {
            code: "embedding-wrong-type".into(),
        }),
    }
}
