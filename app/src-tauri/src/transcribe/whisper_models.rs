//! The local Whisper engine's model files: a fixed catalogue of whisper.cpp's
//! ggml models, which are on disk, how each suits this machine, and their
//! download from Hugging Face (free, no account). They live in
//! `<app data>/models/whisper/`, outside `courses/` and `lectures/`, where
//! folder scans and agents look. Listing reads that directory and the RAM
//! size only; the network is touched only when a download is asked for.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    Model { id: "tiny", label: "Tiny", file: "ggml-tiny.bin", bytes: 77_691_713, ram_bytes: 273_000_000 },
    Model { id: "base", label: "Base", file: "ggml-base.bin", bytes: 147_951_465, ram_bytes: 388_000_000 },
    Model { id: "small", label: "Small", file: "ggml-small.bin", bytes: 487_601_967, ram_bytes: 852_000_000 },
    Model { id: "medium", label: "Medium", file: "ggml-medium.bin", bytes: 1_533_763_059, ram_bytes: 2_100_000_000 },
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
    Model { id: "large-v3", label: "Large v3", file: "ggml-large-v3.bin", bytes: 3_095_033_483, ram_bytes: 3_900_000_000 },
];

/// Near large-v3's accuracy at a fifth of its size, and several times faster.
pub const DEFAULT_MODEL: &str = "large-v3-turbo-q5_0";

/// The largest model [`fits`] recommends to a machine without a GPU.
const SMALL_ON_CPU: &str = "small";

const MODEL_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Silero voice-activity detection for whisper.cpp, under 1 MB; fetched with
/// the first model so a run can skip silence.
pub const VAD_FILE: &str = "ggml-silero-v6.2.0.bin";
const VAD_URL: &str = "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin";

/// Where the models are kept.
pub fn dir() -> PathBuf {
    crate::paths::data_dir().join("models").join("whisper")
}

pub fn find(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

impl Model {
    fn url(&self) -> String {
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
    let cpu_cap = MODELS.iter().position(|m| m.id == SMALL_ON_CPU).unwrap_or(0);
    let default = MODELS.iter().position(|m| m.id == DEFAULT_MODEL).unwrap_or(0);
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
            .ok_or_else(|| "no Whisper model is downloaded — download one in Settings → Transcription".into()),
    }
}

/// Model ids with a download in flight, each with its cancel flag.
static DOWNLOADS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

fn in_flight() -> Vec<String> {
    DOWNLOADS.lock().unwrap().as_ref().map(|d| d.keys().cloned().collect()).unwrap_or_default()
}

/// One download's place in [`DOWNLOADS`], released however it ends so a
/// late cancel cannot poison a retry.
struct Claim(String, Arc<AtomicBool>);

impl Claim {
    fn take(id: &str) -> Result<Self, String> {
        let mut held = DOWNLOADS.lock().unwrap();
        let held = held.get_or_insert_with(HashMap::new);
        if held.contains_key(id) {
            return Err("that model is already downloading".into());
        }
        let flag = Arc::new(AtomicBool::new(false));
        held.insert(id.to_string(), Arc::clone(&flag));
        Ok(Self(id.to_string(), flag))
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(held) = DOWNLOADS.lock().unwrap().as_mut() {
            held.remove(&self.0);
        }
    }
}

/// Ask an in-flight download to stop; false when none runs for `id`.
pub fn cancel(id: &str) -> bool {
    match DOWNLOADS.lock().unwrap().as_ref().and_then(|d| d.get(id)) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// Download `model` into `dir` — and the VAD model first, if it is missing —
/// reporting `(received, total)` bytes of the model as it goes. Ends in
/// `Err(CANCELLED)` when [`cancel`]led.
pub fn download(dir: &Path, model: &Model, progress: impl Fn(u64, u64)) -> Result<PathBuf, String> {
    download_from(dir, model, &model.url(), VAD_URL, progress)
}

pub use crate::echo360::CANCELLED;

fn download_from(
    dir: &Path,
    model: &Model,
    model_url: &str,
    vad_url: &str,
    progress: impl Fn(u64, u64),
) -> Result<PathBuf, String> {
    let claim = Claim::take(model.id)?;
    let cancelled = || claim.1.load(Ordering::Relaxed);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let vad = dir.join(VAD_FILE);
    if !vad.is_file() {
        fetch(vad_url, &vad, 0, &|_, _| {}, &cancelled)
            .map_err(|e| if e == CANCELLED { e } else { format!("the voice-detection model: {e}") })?;
    }
    let dest = dir.join(model.file);
    fetch(model_url, &dest, model.bytes, &progress, &cancelled)?;
    Ok(dest)
}

/// No bytes for this long is a stalled connection, not a slow one; the whole
/// download has no deadline.
const STALL: Duration = Duration::from_secs(60);

/// Stream `url` to `<dest>.part`, then rename it into place. The part is
/// removed on any failure, so a retry starts clean.
fn fetch(
    url: &str,
    dest: &Path,
    expected: u64,
    progress: &dyn Fn(u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, String> {
    let mut part = dest.as_os_str().to_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    let result = (|| {
        let agent = ureq::AgentBuilder::new().timeout_read(STALL).build();
        let response = agent.get(url).call().map_err(|e| match e {
            ureq::Error::Status(status, _) => format!("Hugging Face answered {status} for {url}"),
            e => format!("could not reach Hugging Face: {e}"),
        })?;
        let length = response.header("content-length").and_then(|s| s.parse::<u64>().ok());
        let total = length.unwrap_or(expected);
        let mut reader = response.into_reader();
        let mut file = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let mut buf = vec![0u8; 1 << 16];
        let mut received = 0u64;
        let mut last = Instant::now();
        progress(0, total);
        loop {
            if cancelled() {
                return Err(CANCELLED.to_string());
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("the download broke off after {received} bytes: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", part.display()))?;
            received += n as u64;
            if last.elapsed() >= Duration::from_millis(150) {
                last = Instant::now();
                progress(received, total);
            }
        }
        if length.is_some_and(|length| received != length) {
            return Err(format!("the download stopped at {received} of {total} bytes"));
        }
        file.sync_all().map_err(|e| format!("{}: {e}", part.display()))?;
        drop(file);
        std::fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        progress(received, total);
        Ok(received)
    })();
    if result.is_err() {
        std::fs::remove_file(&part).ok();
    }
    result
}

/// Delete `id`'s file (and any partial download, stopping it first); returns
/// the bytes freed. The VAD model stays: it is under 1 MB.
pub fn delete(dir: &Path, id: &str) -> Result<u64, String> {
    let model = find(id).ok_or_else(|| format!("no Whisper model called {id}"))?;
    cancel(id);
    let mut freed = 0;
    for name in [model.file.to_string(), format!("{}.part", model.file)] {
        let path = dir.join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            freed += meta.len();
        }
    }
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeServer, Reply, Scratch};

    const GB: u64 = 1 << 30;

    fn fit_of(total: u64, gpu: bool) -> Vec<(&'static str, Fit)> {
        MODELS.iter().map(|m| m.id).zip(fits(total, gpu)).collect()
    }

    fn recommended(total: u64, gpu: bool) -> &'static str {
        fit_of(total, gpu).into_iter().find(|(_, f)| *f == Fit::Recommended).unwrap().0
    }

    fn too_large(total: u64, gpu: bool) -> Vec<&'static str> {
        fit_of(total, gpu).into_iter().filter(|(_, f)| *f == Fit::TooLarge).map(|(id, _)| id).collect()
    }

    #[test]
    fn exactly_one_model_is_recommended_on_any_machine() {
        for total in [GB, 2 * GB, 4 * GB, 8 * GB, 16 * GB, 36 * GB, 128 * GB] {
            for gpu in [true, false] {
                let fits = fits(total, gpu);
                assert_eq!(fits.len(), MODELS.len());
                assert_eq!(fits.iter().filter(|f| **f == Fit::Recommended).count(), 1, "{total} {gpu}");
            }
        }
    }

    #[test]
    fn apple_silicon_macs_of_8_16_and_36_gb_get_the_quantised_turbo() {
        for total in [8 * GB, 16 * GB, 36 * GB] {
            assert_eq!(recommended(total, true), DEFAULT_MODEL);
            assert!(too_large(total, true).is_empty(), "{total}");
        }
    }

    #[test]
    fn small_machines_fall_back_and_mark_what_will_not_fit() {
        assert_eq!(recommended(2 * GB, true), "base");
        assert_eq!(too_large(2 * GB, true), vec!["medium", "large-v3-turbo", "large-v3"]);
        assert_eq!(too_large(4 * GB, true), vec!["large-v3"]);
        assert_eq!(recommended(GB / 2, true), "tiny");
    }

    #[test]
    fn without_a_gpu_the_recommendation_stops_at_small() {
        assert_eq!(recommended(8 * GB, false), "small");
        assert_eq!(recommended(36 * GB, false), "small");
        assert_eq!(recommended(2 * GB, false), "base");
    }

    #[test]
    fn the_catalogue_names_ggml_files_on_hugging_face() {
        for m in &MODELS {
            assert_eq!(m.file, format!("ggml-{}.bin", m.id));
            assert_eq!(m.url(), format!("{MODEL_BASE}/ggml-{}.bin", m.id));
        }
        assert!(find(DEFAULT_MODEL).is_some());
    }

    #[test]
    fn a_run_uses_the_chosen_model_else_the_default_else_the_best_on_disk() {
        let dir = Scratch::new("whisper-pick");
        assert!(pick(&dir, None).unwrap_err().contains("no Whisper model is downloaded"));
        std::fs::write(dir.join("ggml-tiny.bin"), b"x").unwrap();
        std::fs::write(dir.join("ggml-small.bin"), b"x").unwrap();
        assert_eq!(pick(&dir, None).unwrap(), dir.join("ggml-small.bin"));
        std::fs::write(dir.join("ggml-large-v3-turbo-q5_0.bin"), b"x").unwrap();
        assert_eq!(pick(&dir, None).unwrap(), dir.join("ggml-large-v3-turbo-q5_0.bin"));
        assert_eq!(pick(&dir, Some("tiny")).unwrap(), dir.join("ggml-tiny.bin"));
        assert!(pick(&dir, Some("medium")).unwrap_err().contains("Medium is not downloaded"));
        assert!(pick(&dir, Some("huge")).unwrap_err().contains("no Whisper model called huge"));
    }

    #[test]
    fn the_listing_reads_the_directory_only() {
        let dir = Scratch::new("whisper-list");
        std::fs::write(dir.join("ggml-base.bin"), b"x").unwrap();
        std::fs::write(dir.join("ggml-small.bin.part"), b"x").unwrap();
        let listed = list(&dir);
        let downloaded: Vec<&str> = listed.models.iter().filter(|m| m.downloaded).map(|m| m.id).collect();
        assert_eq!(downloaded, vec!["base"]);
        assert!(!listed.vad_downloaded);
        let json = serde_json::to_value(&listed).unwrap();
        assert!(json["models"][0].get("ramBytes").is_some());
        assert!(json["models"].as_array().unwrap().iter().any(|m| m["fit"] == "recommended"));
    }

    #[test]
    fn a_download_fetches_the_vad_model_then_the_model_and_reports_progress() {
        let server = FakeServer::start(|hit| match hit.url.as_str() {
            "/vad.bin" => Reply::bytes(vec![1; 1000]),
            "/model.bin" => Reply::bytes(vec![2; 300_000]),
            _ => Reply::status(404, serde_json::json!({})),
        });
        let dir = Scratch::new("whisper-download");
        let model = find("tiny").unwrap();
        let seen = Mutex::new(Vec::new());
        let path = download_from(
            &dir,
            model,
            &format!("{}/model.bin", server.origin()),
            &format!("{}/vad.bin", server.origin()),
            |received, total| seen.lock().unwrap().push((received, total)),
        )
        .unwrap();
        assert_eq!(path, dir.join("ggml-tiny.bin"));
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 300_000);
        assert_eq!(std::fs::metadata(dir.join(VAD_FILE)).unwrap().len(), 1000);
        assert!(!dir.join("ggml-tiny.bin.part").exists());
        let seen = seen.into_inner().unwrap();
        // tiny_http chunks a body this large, so no length: the total is the catalogue's.
        assert_eq!(seen.first(), Some(&(0, model.bytes)));
        assert_eq!(seen.last(), Some(&(300_000, model.bytes)));
        // The VAD model is on disk now, so a second download skips it.
        std::fs::remove_file(&path).unwrap();
        download_from(&dir, model, &format!("{}/model.bin", server.origin()), "http://unused.invalid", |_, _| {})
            .unwrap();
        assert_eq!(server.hits().iter().filter(|h| h.url == "/vad.bin").count(), 1);
    }

    #[test]
    fn a_failed_download_surfaces_and_leaves_nothing_behind() {
        let server = FakeServer::start(|hit| match hit.url.as_str() {
            "/vad.bin" => Reply::bytes(vec![1; 10]),
            _ => Reply::status(404, serde_json::json!({})),
        });
        let dir = Scratch::new("whisper-download-fail");
        let model = find("base").unwrap();
        let error = download_from(
            &dir,
            model,
            &format!("{}/missing.bin", server.origin()),
            &format!("{}/vad.bin", server.origin()),
            |_, _| {},
        )
        .unwrap_err();
        assert!(error.contains("404"), "{error}");
        assert!(!dir.join("ggml-base.bin").exists() && !dir.join("ggml-base.bin.part").exists());
        // The claim was released: the id is free to download again.
        assert!(!in_flight().contains(&"base".to_string()));
    }

    #[test]
    fn a_second_download_of_one_model_is_refused_and_cancel_reaches_the_first() {
        let first = Claim::take("medium").unwrap();
        assert!(Claim::take("medium").is_err());
        assert!(cancel("medium"));
        assert!(first.1.load(Ordering::Relaxed));
        drop(first);
        assert!(!cancel("medium"));
    }

    #[test]
    fn deleting_removes_the_model_and_its_partial() {
        let dir = Scratch::new("whisper-delete");
        std::fs::write(dir.join("ggml-small.bin"), vec![0; 10]).unwrap();
        std::fs::write(dir.join("ggml-small.bin.part"), vec![0; 5]).unwrap();
        std::fs::write(dir.join(VAD_FILE), b"v").unwrap();
        assert_eq!(delete(&dir, "small").unwrap(), 15);
        assert!(!dir.join("ggml-small.bin").exists());
        assert!(dir.join(VAD_FILE).exists());
        assert_eq!(delete(&dir, "small").unwrap(), 0);
        assert!(delete(&dir, "nope").is_err());
    }

    #[test]
    fn this_machines_ram_is_read() {
        if cfg!(unix) {
            assert!(total_memory().unwrap() > GB);
        }
    }
}
