//! Reuse platform TLS, cancellation and credential storage. API keys never cross this boundary.
use std::{collections::BTreeMap, sync::Arc};

#[derive(uniffi::Record)]
pub struct VeniceHeader {
    pub name: String,
    pub value: String,
}
#[derive(uniffi::Record)]
pub struct VeniceRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<VeniceHeader>,
    pub body: Vec<u8>,
}
#[derive(uniffi::Record)]
pub struct VeniceResponse {
    pub status: u16,
    pub headers: Vec<VeniceHeader>,
    pub body: Vec<u8>,
}

#[uniffi::export(callback_interface)]
pub trait VeniceHttp: Send + Sync {
    /// Status 0: transport failure; 499: cancelled. Bound reads to 8 MiB, disable redirects,
    /// keep TLS validation, and never attach the Venice key to a collateral request.
    fn execute(&self, request: VeniceRequest) -> VeniceResponse;
}

struct HttpBridge(Arc<dyn VeniceHttp>);
impl iris_cloud::Transport for HttpBridge {
    fn execute(&self, r: iris_cloud::HttpRequest) -> iris_cloud::Result<iris_cloud::HttpResponse> {
        let r = self.0.execute(VeniceRequest {
            method: r.method,
            url: r.url,
            body: r.body,
            headers: r
                .headers
                .into_iter()
                .map(|(name, value)| VeniceHeader { name, value })
                .collect(),
        });
        match r.status {
            0 => return Err(iris_cloud::Error::Network),
            499 => return Err(iris_cloud::Error::Cancelled),
            _ => {}
        }
        Ok(iris_cloud::HttpResponse {
            status: r.status,
            body: r.body,
            headers: r
                .headers
                .into_iter()
                .map(|h| (h.name, h.value))
                .collect::<BTreeMap<_, _>>(),
        })
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum VeniceError {
    #[error("{code}")]
    Failed { code: String },
}
impl From<iris_cloud::Error> for VeniceError {
    fn from(e: iris_cloud::Error) -> Self {
        Self::Failed {
            code: e.to_string(),
        }
    }
}
fn encode<T: serde::Serialize>(value: &T) -> Result<String, VeniceError> {
    serde_json::to_string(value).map_err(|_| VeniceError::from(iris_cloud::Error::Protocol))
}

#[derive(uniffi::Object)]
pub struct VeniceConnection {
    client: iris_cloud::Client,
}
#[uniffi::export]
impl VeniceConnection {
    #[uniffi::constructor]
    pub fn new(http: Box<dyn VeniceHttp>) -> Arc<Self> {
        Arc::new(Self {
            client: iris_cloud::Client::new(Arc::new(HttpBridge(Arc::from(http)))),
        })
    }
    pub fn models(&self) -> Result<String, VeniceError> {
        encode(&self.client.models()?)
    }
    pub fn inspect(&self, model: String) -> Result<String, VeniceError> {
        encode(&self.client.inspect(&model)?)
    }
    pub fn complete(
        &self,
        model: String,
        trusted_fingerprint: String,
        system: String,
        history: String,
        user: String,
    ) -> Result<String, VeniceError> {
        encode(
            &self
                .client
                .complete(&model, &trusted_fingerprint, &system, &history, &user)?,
        )
    }
}

// ---- the AI integration, independent of the provider -------------------------------------------------------

/// System and user message to tidy up entries: JSON `{"system","user"}`. `drafts` is JSON `[{"id","kind","title","collection"}]`.
#[uniffi::export]
pub fn organize_prompt(language: String, sentence: String, drafts: String, lists: Vec<String>, rules: String, allow_new_lists: bool) -> String {
    let drafts: Vec<iris_cloud::organize::Draft> = serde_json::from_str(&drafts).unwrap_or_default();
    let (system, user) = iris_cloud::organize::prompt(&language, &sentence, &drafts, &lists, &rules, allow_new_lists);
    serde_json::json!({"system": system, "user": user}).to_string()
}

/// The checked changes of the AI's answer: JSON `[{"id","title","collection"}]`, empty when nothing usable.
#[uniffi::export]
pub fn organize_apply(answer: String, sentence: String, drafts: String, lists: Vec<String>, allow_new_lists: bool) -> String {
    let drafts: Vec<iris_cloud::organize::Draft> = serde_json::from_str(&drafts).unwrap_or_default();
    serde_json::to_string(&iris_cloud::organize::apply(&answer, &sentence, &drafts, &lists, allow_new_lists)).unwrap_or_else(|_| "[]".into())
}

/// "Ordne meine Erinnerungen": "reminder", "item", "note" or "all"; none for other sentences.
#[uniffi::export]
pub fn organize_request(text: String) -> Option<String> {
    iris_cloud::organize::request(&text).map(str::to_string)
}

/// The memory text in its current form ("key: value" lines; earlier "- fact" lists are taken over).
#[uniffi::export]
pub fn memory_normalize(text: String) -> String {
    iris_cloud::memory::render(&iris_cloud::memory::parse(&text))
}

/// The bounded memory block for a session's context.
#[uniffi::export]
pub fn memory_for_prompt(text: String) -> String {
    iris_cloud::memory::for_prompt(&iris_cloud::memory::parse(&text))
}

/// Applies the changes a reply proposed: JSON `{"text": new memory text, "applied": [{"op","key","value"}]}`.
#[uniffi::export]
pub fn memory_apply(text: String, ops: String) -> String {
    let (entries, applied) = iris_cloud::memory::apply(&iris_cloud::memory::parse(&text), &ops);
    serde_json::json!({"text": iris_cloud::memory::render(&entries), "applied": applied}).to_string()
}

/// The entries a question is about: JSON `[{"key","value"}]`.
#[uniffi::export]
pub fn memory_recall(text: String, question: String) -> String {
    serde_json::to_string(&iris_cloud::memory::recall(&iris_cloud::memory::parse(&text), &question)).unwrap_or_else(|_| "[]".into())
}

/// Whether a memory question asks about something specific (nothing found then means nothing known).
#[uniffi::export]
pub fn memory_specific(question: String) -> bool {
    iris_cloud::memory::specific(&question)
}
