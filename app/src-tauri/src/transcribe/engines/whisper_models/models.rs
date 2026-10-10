//! The catalogue: which models exist, which are on disk, and how each fits this machine.

use std::path::{Path, PathBuf};

use super::download::in_flight;

pub struct Model {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    /// The file's size on Hugging Face.
    pub bytes: u64,
    /// Roughly what a run holds in memory: whisper.cpp's README table, and for
    /// turbo (four decoder layers, so a far smaller KV cache) its weights plus
    /// about 0.4 GB.
    pub ram_bytes: u64,
}

/// Worst to best transcription; "largest" below means latest in this list.
pub const MODELS: [Model; 7] = [
    Model {
        id: "tiny",
        label: "Tiny",
        file: "ggml-tiny.bin",
        bytes: 77_691_713,
        ram_bytes: 273_000_000,
    },
    Model {
        id: "base",
        label: "Base",
        file: "ggml-base.bin",
        bytes: 147_951_465,
        ram_bytes: 388_000_000,
    },
    Model {
        id: "small",
        label: "Small",
        file: "ggml-small.bin",
        bytes: 487_601_967,
        ram_bytes: 852_000_000,
    },
    Model {
        id: "medium",
        label: "Medium",
        file: "ggml-medium.bin",
        bytes: 1_533_763_059,
        ram_bytes: 2_100_000_000,
    },
    Model {
        id: "large-v3-turbo-q5_0",
        label: "Large v3 Turbo (quantised)",
        file: "ggml-large-v3-turbo-q5_0.bin",
        bytes: 574_041_195,
        ram_bytes: 1_000_000_000,
    },
    Model {
        id: "large-v3-turbo",
        label: "Large v3 Turbo",
        file: "ggml-large-v3-turbo.bin",
        bytes: 1_624_555_275,
        ram_bytes: 2_000_000_000,
    },
    Model {
        id: "large-v3",
        label: "Large v3",
        file: "ggml-large-v3.bin",
        bytes: 3_095_033_483,
        ram_bytes: 3_900_000_000,
    },
];

/// Near large-v3's accuracy at a fifth of its size, and several times faster.
pub const DEFAULT_MODEL: &str = "large-v3-turbo-q5_0";

/// The largest model [`fits`] recommends to a machine without a GPU.
const SMALL_ON_CPU: &str = "small";

pub(super) const MODEL_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Silero voice-activity detection for whisper.cpp, under 1 MB; fetched with
/// the first model so a run can skip silence.
pub const VAD_FILE: &str = "ggml-silero-v6.2.0.bin";
pub(super) const VAD_URL: &str =
    "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin";

/// Where the models are kept.
pub fn dir() -> PathBuf {
    crate::library::paths::data_dir()
        .join("models")
        .join("whisper")
}

pub fn find(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

impl Model {
    pub(super) fn url(&self) -> String {
        format!("{MODEL_BASE}/{}", self.file)
    }
}

/// whisper.cpp runs on the GPU through Metal on Apple Silicon only.
pub const GPU: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    Recommended,
    Ok,
    TooLarge,
}

/// How each of [`MODELS`] suits a machine with `total` bytes of RAM, in order.
/// Too large: it needs more than half the RAM. Recommended, exactly one: the
/// default if it needs at most a quarter and a GPU runs it; else the largest
/// needing at most a quarter — no larger than `small` without a GPU, where the
/// large models decode slowly — else `tiny`. Every other model is ok.
pub fn fits(total: u64, gpu: bool) -> Vec<Fit> {
    let comfortable = |m: &Model| m.ram_bytes <= total / 4;
    let cpu_cap = MODELS
        .iter()
        .position(|m| m.id == SMALL_ON_CPU)
        .unwrap_or(0);
    let default = MODELS
        .iter()
        .position(|m| m.id == DEFAULT_MODEL)
        .unwrap_or(0);
    let recommended = if gpu && comfortable(&MODELS[default]) {
        default
    } else {
        MODELS
            .iter()
            .enumerate()
            .filter(|&(i, m)| comfortable(m) && (gpu || i <= cpu_cap))
            .map(|(i, _)| i)
            .last()
            .unwrap_or(0)
    };
    MODELS
        .iter()
        .enumerate()
        .map(|(i, m)| match () {
            _ if i == recommended => Fit::Recommended,
            _ if m.ram_bytes > total / 2 => Fit::TooLarge,
            _ => Fit::Ok,
        })
        .collect()
}

/// Physical RAM, from `hw.memsize` on macOS.
#[cfg(target_os = "macos")]
pub fn total_memory() -> Option<u64> {
    let mut bytes: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    let rc = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            &mut bytes as *mut u64 as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0 && bytes > 0).then_some(bytes)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn total_memory() -> Option<u64> {
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (pages > 0 && size > 0).then(|| pages as u64 * size as u64)
}

#[cfg(not(unix))]
pub fn total_memory() -> Option<u64> {
    None
}

/// What the fit is judged against when the RAM size cannot be read.
const ASSUMED_RAM: u64 = 8 << 30;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub id: &'static str,
    pub label: &'static str,
    pub bytes: u64,
    pub ram_bytes: u64,
    pub downloaded: bool,
    /// A download of it is in flight in this process.
    pub downloading: bool,
    pub fit: Fit,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalogue {
    pub total_ram_bytes: Option<u64>,
    pub gpu: bool,
    pub vad_downloaded: bool,
    /// [`DEFAULT_MODEL`], so the UI can show what [`pick`] would choose.
    pub default_model: &'static str,
    pub models: Vec<Listed>,
}

/// Every model with what is on disk in `dir`. Local only.
pub fn list(dir: &Path) -> Catalogue {
    let total = total_memory();
    let fits = fits(total.unwrap_or(ASSUMED_RAM), GPU);
    let downloading = in_flight();
    Catalogue {
        total_ram_bytes: total,
        gpu: GPU,
        vad_downloaded: dir.join(VAD_FILE).is_file(),
        default_model: DEFAULT_MODEL,
        models: MODELS
            .iter()
            .zip(fits)
            .map(|(m, fit)| Listed {
                id: m.id,
                label: m.label,
                bytes: m.bytes,
                ram_bytes: m.ram_bytes,
                downloaded: dir.join(m.file).is_file(),
                downloading: downloading.iter().any(|id| id == m.id),
                fit,
            })
            .collect(),
    }
}

/// The model file a run uses: `chosen` when set, which must be on disk;
/// unset, the default if downloaded, else the largest downloaded.
pub fn pick(dir: &Path, chosen: Option<&str>) -> Result<PathBuf, String> {
    let on_disk = |m: &Model| dir.join(m.file).is_file().then(|| dir.join(m.file));
    match chosen {
        Some(id) => {
            let model = find(id).ok_or_else(|| format!("no Whisper model called {id}"))?;
            on_disk(model).ok_or_else(|| {
                format!("the Whisper model {} is not downloaded — download it in Settings → Transcription", model.label)
            })
        }
        None => find(DEFAULT_MODEL)
            .and_then(on_disk)
            .or_else(|| MODELS.iter().rev().find_map(on_disk))
            .ok_or_else(|| {
                "no Whisper model is downloaded — download one in Settings → Transcription".into()
            }),
    }
}
