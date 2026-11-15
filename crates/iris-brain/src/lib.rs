//! "Iris Brain": a pruned multilingual MiniLM fine-tuned on Iris' acts. [`Brain::load`] reads a directory
//! holding `brain.onnx`, `meta.txt` and `pieces.tsv`.
//!
//! The network runs on ONNX Runtime, loaded at run time from the library the app already ships for speech
//! (`libonnxruntime.so`, 1.28). A pure-Rust runtime was measured first: about five times slower and five
//! times the CPU time per sentence, so it was not taken. One intra-op thread without spinning: as fast as
//! two threads for one short sentence, at half the CPU time (battery).

mod context;

pub use context::Context;

use iris_core::{Classifier, ConversationContext, Prediction};
use std::collections::HashMap;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use unicode_normalization::UnicodeNormalization;

pub const MAX_CTX: usize = 52;
pub const MAX_UTT: usize = 28;

/// Unigram tokenizer over the pruned piece table. Gives the ids of `nlu/brain/tok.py`.
pub struct Tokenizer {
    id: HashMap<String, usize>,
    score: Vec<f64>,
    max_len: usize,
    unk: usize,
}

impl Tokenizer {
    pub fn parse(tsv: &str) -> Self {
        let mut id = HashMap::new();
        let mut score = Vec::new();
        let mut max_len = 0;
        for line in tsv.lines().filter(|l| !l.is_empty()) {
            let (piece, s) = line.rsplit_once('\t').expect("piece and score");
            max_len = max_len.max(piece.chars().count());
            id.insert(piece.to_string(), score.len());
            score.push(s.parse::<f64>().expect("score"));
        }
        let unk = id["<unk>"];
        Self { id, score, max_len, unk }
    }

    fn norm(text: &str) -> Vec<String> {
        text.nfkc().collect::<String>().to_lowercase().split_whitespace().map(str::to_string).collect()
    }

    fn word(&self, w: &str) -> Vec<usize> {
        let cps: Vec<char> = std::iter::once('▁').chain(w.chars()).collect();
        let n = cps.len();
        let mut best = vec![-1e18f64; n + 1];
        let mut back = vec![0usize; n + 1];
        best[0] = 0.0;
        for i in 0..n {
            if best[i] <= -1e17 {
                continue;
            }
            for j in i + 1..=n.min(i + self.max_len) {
                let piece: String = cps[i..j].iter().collect();
                if let Some(&pid) = self.id.get(&piece) {
                    let v = best[i] + self.score[pid];
                    if v > best[j] {
                        best[j] = v;
                        back[j] = i;
                    }
                }
            }
            if best[i] - 100.0 > best[i + 1] {
                best[i + 1] = best[i] - 100.0;
                back[i + 1] = i;
            }
        }
        let mut out = Vec::new();
        let mut j = n;
        while j > 0 {
            let i = back[j];
            let piece: String = cps[i..j].iter().collect();
            out.push(self.id.get(&piece).copied().unwrap_or(self.unk));
            j = i;
        }
        out.reverse();
        out
    }

    pub fn encode(&self, text: &str) -> Vec<usize> {
        Self::norm(text).iter().flat_map(|w| self.word(w)).collect()
    }
}

/// Label sets and where each head starts in the output vector.
pub struct Meta {
    pub labels: Vec<(String, Vec<String>)>,
    pub offsets: HashMap<String, usize>,
}

impl Meta {
    pub fn parse(text: &str) -> Self {
        let (mut labels, mut offsets) = (Vec::new(), HashMap::new());
        for line in text.lines() {
            let p: Vec<&str> = line.splitn(3, ':').collect();
            if p.len() == 3 && p[0] != "total" {
                offsets.insert(p[0].to_string(), p[1].parse().expect("offset"));
                labels.push((p[0].to_string(), p[2].split(',').map(str::to_string).collect()));
            }
        }
        Self { labels, offsets }
    }
}

#[derive(Debug, Clone)]
pub struct Scored {
    pub label: String,
    pub p: f32,
}

/// What the network made of a sentence: the three likeliest acts and the best label of every other head.
#[derive(Debug, Clone)]
pub struct Understanding {
    pub acts: Vec<Scored>,
    pub heads: Vec<(String, Scored)>,
}

/// Why the network could not be loaded or run. Only the kind of failure, never a sentence.
#[derive(Debug, Clone, PartialEq)]
pub struct BrainError(pub String);

impl std::fmt::Display for BrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BrainError {}

pub type BrainResult<T> = Result<T, BrainError>;

fn err(e: impl std::fmt::Display) -> BrainError {
    BrainError(e.to_string())
}

/// ONNX Runtime allows one environment per process; it is made once, from the first library path given.
static ENVIRONMENT: OnceLock<Result<ort::environment::Environment, BrainError>> = OnceLock::new();

fn environment(library: &Path) -> BrainResult<&'static ort::environment::Environment> {
    ENVIRONMENT
        .get_or_init(|| {
            let builder = ort::init_from(library).map_err(err)?;
            builder.with_name("iris-brain").with_telemetry(false).build().map_err(err)
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Where ONNX Runtime is: `IRIS_ORT_LIB` if set (tests and the desktop tool), else the library by its name,
/// which on Android is found among the app's own native libraries.
pub fn default_library() -> std::path::PathBuf {
    match std::env::var("IRIS_ORT_LIB") {
        Ok(p) if !p.is_empty() => p.into(),
        _ => format!("{}onnxruntime{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX).into(),
    }
}

pub struct Brain {
    session: Mutex<Session>,
    tok: Tokenizer,
    meta: Meta,
}

impl Brain {
    /// Loads the network from `dir` with ONNX Runtime from [`default_library`].
    pub fn load(dir: &Path) -> BrainResult<Self> {
        Self::load_with(dir, &default_library())
    }

    /// Loads the network from `dir` with ONNX Runtime from `library`.
    pub fn load_with(dir: &Path, library: &Path) -> BrainResult<Self> {
        let tok = Tokenizer::parse(&std::fs::read_to_string(dir.join("pieces.tsv")).map_err(err)?);
        let meta = Meta::parse(&std::fs::read_to_string(dir.join("meta.txt")).map_err(err)?);
        let env = environment(library)?;
        let session = Session::builder(env)
            .map_err(err)?
            .with_optimization_level(GraphOptimizationLevel::All)
            .map_err(err)?
            .with_intra_threads(1)
            .map_err(err)?
            .with_inter_threads(1)
            .map_err(err)?
            .with_intra_op_spinning(false)
            .map_err(err)?
            .commit_from_file(dir.join("brain.onnx"))
            .map_err(err)?;
        Ok(Self { session: Mutex::new(session), tok, meta })
    }

    /// Token ids and the index where the utterance starts, as the network expects them.
    pub fn encode(&self, ctx: &Context, text: &str) -> (Vec<i64>, usize) {
        let c: Vec<usize> = self.tok.encode(&ctx.compose()).into_iter().take(MAX_CTX).collect();
        let u: Vec<usize> = self.tok.encode(text).into_iter().take(MAX_UTT).collect();
        let mut ids = vec![0i64];
        ids.extend(c.iter().map(|x| *x as i64));
        ids.extend([2, 2]);
        ids.extend(u.iter().map(|x| *x as i64));
        ids.push(2);
        (ids, 1 + c.len() + 2)
    }

    pub fn logits(&self, ids: &[i64], ustart: usize) -> BrainResult<Vec<f32>> {
        let n = ids.len();
        let ids_t = Tensor::from_array(([1usize, n], ids.to_vec())).map_err(err)?;
        let mask_t = Tensor::from_array(([1usize, n], vec![1i64; n])).map_err(err)?;
        let us_t = Tensor::from_array(([1usize], vec![ustart as i64])).map_err(err)?;
        let mut session = self.session.lock().map_err(|_| BrainError("network lock poisoned".into()))?;
        let out = session.run(ort::inputs!["ids" => ids_t, "mask" => mask_t, "ustart" => us_t]).map_err(err)?;
        let (_, logits) = out[0].try_extract_tensor::<f32>().map_err(err)?;
        Ok(logits.to_vec())
    }

    pub fn decode(&self, logits: &[f32]) -> Understanding {
        let slice = |name: &str| -> Vec<Scored> {
            let (_, labels) = self.meta.labels.iter().find(|(n, _)| n == name).expect("head");
            let o = self.meta.offsets[name];
            let max = labels.iter().enumerate().map(|(k, _)| logits[o + k]).fold(f32::NEG_INFINITY, f32::max);
            let e: Vec<f64> = (0..labels.len()).map(|k| ((logits[o + k] - max) as f64).exp()).collect();
            let sum: f64 = e.iter().sum();
            let mut all: Vec<Scored> = labels.iter().enumerate().map(|(k, l)| Scored { label: l.clone(), p: (e[k] / sum) as f32 }).collect();
            all.sort_by(|a, b| b.p.partial_cmp(&a.p).unwrap());
            all
        };
        let heads = self.meta.labels.iter().filter(|(n, _)| n != "act").map(|(n, _)| (n.clone(), slice(n).remove(0))).collect();
        Understanding { acts: slice("act").into_iter().take(3).collect(), heads }
    }

    pub fn understand(&self, ctx: &Context, text: &str) -> BrainResult<Understanding> {
        let (ids, us) = self.encode(ctx, text);
        Ok(self.decode(&self.logits(&ids, us)?))
    }
}

impl Classifier for Brain {
    /// The likeliest acts for a sentence, told what was said and asked before.
    fn classify(&self, text: &str, _lang: &str) -> Vec<Prediction> {
        self.classify_with(text, _lang, &ConversationContext::default())
    }

    fn classify_with(&self, text: &str, _lang: &str, c: &ConversationContext) -> Vec<Prediction> {
        let ctx = Context { last: c.last.clone(), last2: c.last2.clone(), expect: c.expect.clone(), slot: c.slot.clone(), age: c.age, focus: -1, ..Context::default() };
        match self.understand(&ctx, text) {
            Ok(u) => u.acts.into_iter().map(|s| Prediction { label: s.label, p: s.p }).collect(),
            Err(_) => Vec::new(),
        }
    }
}
