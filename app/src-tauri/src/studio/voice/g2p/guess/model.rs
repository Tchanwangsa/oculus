//! The `g2p-en-us` pack: the guesser's encoder, decoder and alphabet, and
//! misaki's US dictionaries, downloaded on request to
//! `<app data>/models/voice/` beside the Kokoro voice, never bundled.

use std::path::PathBuf;

use crate::runtime::onnx::{Pack, PackFile};

pub const ENCODER_FILE: &str = "g2p-encoder.onnx";
pub const DECODER_FILE: &str = "g2p-decoder.onnx";
pub const CONFIG_FILE: &str = "g2p-config.json";
pub const GOLD_FILE: &str = "us_gold.json";
pub const SILVER_FILE: &str = "us_silver.json";

/// PeterReid/graphemes_to_phonemes_en_us (BART, Apache-2.0 in its model
/// card) as exported in that repo's PR #1, pinned to the PR's commit. It
/// decodes the oracle's 305 verify words identically to our own export. The
/// dictionaries are misaki's (Apache-2.0) at the last commit that changed
/// them, byte-identical to misaki 0.9.4's.
pub const PACK: Pack = Pack {
    id: "g2p-en-us",
    files: &[
        PackFile {
            file: ENCODER_FILE,
            url: "https://huggingface.co/PeterReid/graphemes_to_phonemes_en_us/resolve/9470bafd46d1e5c05225f2942853b1de90bc9658/onnx/encoder_model.onnx",
            bytes: 1_414_890,
            sha256: "5419f10161ea94c960c24890b4a125f44295d80ed56dd80a43e3d90dd75e01ae",
        },
        PackFile {
            file: DECODER_FILE,
            url: "https://huggingface.co/PeterReid/graphemes_to_phonemes_en_us/resolve/9470bafd46d1e5c05225f2942853b1de90bc9658/onnx/decoder_model.onnx",
            bytes: 1_750_816,
            sha256: "c091bca25466cf3c29b2c720c804774e26e9244b856f1c92c08308ac54d5201e",
        },
        PackFile {
            file: CONFIG_FILE,
            url: "https://huggingface.co/PeterReid/graphemes_to_phonemes_en_us/resolve/9470bafd46d1e5c05225f2942853b1de90bc9658/config.json",
            bytes: 1_257,
            sha256: "8deb3537fb29c63cd9f20d75515ae06e4c92f1b6db0703a2d45bca95b33a53a4",
        },
        PackFile {
            file: GOLD_FILE,
            url: "https://raw.githubusercontent.com/hexgrad/misaki/e820629b96334db28227df37f280e4836d46fadb/misaki/data/us_gold.json",
            bytes: 3_000_469,
            sha256: "dc414872a49a28ae6c141463d502fd945f3b2fde040484fdc47d00cc4612686f",
        },
        PackFile {
            file: SILVER_FILE,
            url: "https://raw.githubusercontent.com/hexgrad/misaki/e820629b96334db28227df37f280e4836d46fadb/misaki/data/us_silver.json",
            bytes: 3_099_517,
            sha256: "de8f67be911bb6c659187b4a65fd966b6a30e56350e0f790d763210b053ac475",
        },
    ],
};

/// Where the pack is kept.
pub fn dir() -> PathBuf {
    crate::library::paths::data_dir()
        .join("models")
        .join("voice")
}
