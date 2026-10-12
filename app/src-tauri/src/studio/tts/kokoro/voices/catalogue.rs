//! The 20 US English voices of onnx-community/Kokoro-82M-v1.0-ONNX at the
//! commit the model is pinned to. Grades are the "Overall Grade" column of
//! hexgrad/Kokoro-82M's VOICES.md; each sha256 is of the file at that
//! commit.

use super::super::model::VOICE_BYTES;
use super::{Accent, Gender, Voice};
use crate::runtime::onnx::{Pack, PackFile};

macro_rules! voice {
    ($id:literal, $name:literal, $gender:ident, $grade:literal, $sha256:literal) => {
        Voice {
            id: $id,
            name: $name,
            accent: Accent::American,
            gender: Gender::$gender,
            grade: $grade,
            pack: Pack {
                id: concat!("voice-", $id),
                files: &[PackFile {
                    file: concat!($id, ".bin"),
                    url: concat!(
                        "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/voices/",
                        $id,
                        ".bin"
                    ),
                    bytes: VOICE_BYTES,
                    sha256: $sha256,
                }],
            },
        }
    };
}

/// Women first, then men, each by name. One line a voice, for reading down.
#[rustfmt::skip]
pub static VOICES: [Voice; 20] = [
    voice!("af_alloy", "Alloy", Female, "C", "c4a6b876047fd7fb472edf4ebd63cfac7c3b958a7cae7c106e8f038ca6308c45"),
    voice!("af_aoede", "Aoede", Female, "C+", "4a004c33430762e2461eedb2013fad808ef4ab3121f5300f554476caf58d8361"),
    voice!("af_bella", "Bella", Female, "A-", "f69d836209b78eb8c66e75e3cda491e26ea838a3674257e9d4e5703cbaf55c8b"),
    voice!("af_heart", "Heart", Female, "A", "d583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b"),
    voice!("af_jessica", "Jessica", Female, "D", "a240a5e3c15b43563d6e923bdca8ef5613a23471d9b77653694012435df23bd8"),
    voice!("af_kore", "Kore", Female, "C+", "9be5221b6a941c04b561959b8ff0b06e809444dcc4ab7e75a7b23606f691819e"),
    voice!("af_nicole", "Nicole", Female, "B-", "cd2191ab31b914ed7b318416b0e4440fdf392ddad9106a060819aa600a64f59a"),
    voice!("af_nova", "Nova", Female, "C", "18778272caa0d0eebaea251c35fd635f038434f9eee5e691d02a174bd328414f"),
    voice!("af_river", "River", Female, "D", "00a2bcf82b1d86e8f19902ede58c65ccf6c0e43b44b7d74fad54e5d8933c9c30"),
    voice!("af_sarah", "Sarah", Female, "C+", "4409fbc125afabacc615d94db5398d847006a737b0247d6892b7a9a0007a2f0a"),
    voice!("af_sky", "Sky", Female, "C-", "4435255c9744f3f31659e0d714ab7689bf65d9e77ec1cce060f083912614f0b9"),
    voice!("am_adam", "Adam", Male, "F+", "162b035ed91cfc48b6046982184c645f72edcdd1b82843347f605d7bf7b15716"),
    voice!("am_echo", "Echo", Male, "D", "3968b92c3c4cd1c4416dbded36c13eaa388a90d5788d02a13e4d781f5f8cf3c3"),
    voice!("am_eric", "Eric", Male, "D", "e8b5be17edd1e3636901ce7598baafe2dc8dd8ff707a0c23bf9e461add7e2832"),
    voice!("am_fenrir", "Fenrir", Male, "C+", "c27989f741f7ee34d273a39d8a595cc0837d35f5ced9a29b7cc162614616df43"),
    voice!("am_liam", "Liam", Male, "D", "52403be32fd047c6a44517cb0bcd6b134f2a18baa73e70ef41651e0eab921ade"),
    voice!("am_michael", "Michael", Male, "C+", "1d1f21dd8da39c30705cd4c75d039d265e9bc4a2a93ed09bc9e1b1225eb95ba1"),
    voice!("am_onyx", "Onyx", Male, "D", "da5d135b424164916d75a68ffb4c2abce3d7d5ccc82dd1ee6cf447ce286145e6"),
    voice!("am_puck", "Puck", Male, "C+", "fcf73c989033e9233e0b98713eca600c8c74dcc1614b37009d5450ff4a2274a0"),
    voice!("am_santa", "Santa", Male, "D-", "61150cf726ab6c5ed7a99f90a304f91f5a72c00c592e89ec94e5df11c319227a"),
];
