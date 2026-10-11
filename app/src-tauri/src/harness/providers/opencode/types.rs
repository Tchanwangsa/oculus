//! The bridge's public shapes: spawn and session options, and the rows
//! Settings reads (models, providers, sign-in methods).

use std::path::PathBuf;

use serde::Serialize;

use crate::harness::{RawLog, Sink};

pub struct OpencodeSpawn {
    pub bin: PathBuf,
    /// The library's `agents/` folder: session directory, opencode's project
    /// root, and the only place the agent may write.
    pub directory: PathBuf,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
    /// Takes the events that name no session.
    pub default_sink: Option<Sink>,
}

/// How to open a session. `model` is `providerID/id`; `variant` is a reasoning
/// level the model itself declared (none do as of 1.18.x).
#[derive(Default, Clone)]
pub struct OpencodeSessionOpts {
    pub model: Option<String>,
    pub variant: Option<String>,
    /// The per-thread part of the brief, sent ahead of the first message.
    pub brief: String,
    /// [`super::AGENT`], [`super::NAMING_AGENT`], [`super::WRITER_AGENT`] or [`super::LECTURE_END_AGENT`].
    pub agent: &'static str,
}

/// One row of the catalogue, in the shape `app/src/lib/harness/models.ts`'s
/// `OpencodeModel` reads.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// `providerID/id`; see [`super::split_model`].
    pub id: String,
    pub display_name: String,
    pub description: String,
    /// The model's own reasoning levels (empty for every model in 1.18.x).
    pub variants: Vec<String>,
    pub default_variant: Option<String>,
    pub is_default: bool,
    /// What the catalogue claims; `unusableReason` in
    /// `app/src/lib/harness/opencodeCatalogue.ts` decides what that means.
    pub tool_call: bool,
    pub text_input: bool,
    pub text_output: bool,
    /// What Settings → opencode's model table shows.
    pub facts: ModelFacts,
}

/// A model's price, limits and capabilities as the catalogue states them —
/// `ModelFacts` in `app/src/lib/harness/models.ts`. An absent field is `None`:
/// unknown, which a price column must not draw as zero.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelFacts {
    pub cost: Option<ModelCost>,
    pub context: Option<u64>,
    pub max_output: Option<u64>,
    pub reasoning: bool,
    pub attachment: bool,
    /// Input modalities besides text, in a fixed order.
    pub inputs: Vec<String>,
    pub release_date: Option<String>,
    pub family: Option<String>,
}

/// USD per million tokens.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

/// One row of Settings → AI's provider list, in the shape
/// `app/src/lib/harness/opencodeAuth.ts` reads.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    /// `env` | `config` | `custom` | `api`. `config` is declared in an
    /// `opencode.json`, not signed in to, so it has no credential to remove.
    pub source: String,
    /// Env vars the provider reads a key from. A hint only: the harness
    /// strips provider keys from the child's environment.
    pub env: Vec<String>,
    pub model_count: usize,
    pub connected: bool,
    /// Never empty: `default_method` stands in for a provider that
    /// declares none.
    pub methods: Vec<AuthMethod>,
}

/// What a provider read answers: the list, and whether it is current.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderList {
    pub providers: Vec<ProviderInfo>,
    /// A refresh was skipped because a turn was running: the credential was
    /// written, but `connected` is still the state before it.
    pub stale: bool,
}

/// One way in, as opencode declares it: a form spec the dialog draws from.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethod {
    /// Position in the provider's `/provider/auth` array — the only name the
    /// OAuth endpoints have for a method, so `parse_methods` never reorders.
    pub index: usize,
    /// `oauth` | `api`.
    pub kind: String,
    pub label: String,
    /// Extra fields. An `api` method's key is not one of them.
    pub prompts: Vec<AuthPrompt>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthPrompt {
    /// `text` | `select`.
    pub kind: String,
    pub key: String,
    pub message: String,
    pub placeholder: Option<String>,
    /// Empty unless `kind` is `select`.
    pub options: Vec<AuthOption>,
    /// Shows this field only when another answer matches.
    pub when: Option<AuthWhen>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthOption {
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthWhen {
    pub key: String,
    /// `eq` | `neq`.
    pub op: String,
    pub value: String,
}

/// What `POST …/oauth/authorize` answers. `method` is `auto` when the server
/// finishes the flow itself, `code` when the student pastes something back.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Authorization {
    pub url: String,
    pub method: String,
    pub instructions: String,
}
