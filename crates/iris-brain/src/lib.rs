//! "Iris Brain": a pruned multilingual MiniLM fine-tuned on Iris' acts, run without any native
//! library. The model files are not part of this repository; [`Brain::load`] reads a directory
//! holding `brain.onnx`, `meta.txt` and `pieces.tsv`.

mod context;

pub use context::Context;

use iris_core::{Classifier, Prediction};
use std::collections::HashMap;
use std::path::Path;
use tract_onnx::prelude::*;
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

type Model = std::sync::Arc<TypedSimplePlan>;

pub struct Brain {
    model: Model,
    tok: Tokenizer,
    meta: Meta,
}

impl Brain {
    pub fn load(dir: &Path) -> TractResult<Self> {
        let tok = Tokenizer::parse(&std::fs::read_to_string(dir.join("pieces.tsv"))?);
        let meta = Meta::parse(&std::fs::read_to_string(dir.join("meta.txt"))?);
        // The batch is always one sentence, and the shapes the exporter wrote down for inner nodes (with a
        // symbolic batch) get in the way of that: drop them, the input facts below say what is needed.
        let onnx = tract_onnx::onnx();
        let mut proto = onnx.proto_model_for_path(dir.join("brain.onnx"))?;
        if let Some(g) = proto.graph.as_mut() {
            g.value_info.clear();
        }
        let mut graph = onnx.model_for_proto_model(&proto)?;
        let t = graph.symbols.sym("t");
        let seq = || InferenceFact::dt_shape(i64::datum_type(), tvec!(TDim::from(1), TDim::from(t.clone())));
        graph = graph.with_input_fact(0, seq())?.with_input_fact(1, seq())?.with_input_fact(2, InferenceFact::dt_shape(i64::datum_type(), tvec!(TDim::from(1))))?;
        let model = graph.into_optimized()?.into_runnable()?;
        Ok(Self { model, tok, meta })
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

    pub fn logits(&self, ids: &[i64], ustart: usize) -> TractResult<Vec<f32>> {
        let n = ids.len();
        let ids_t: Tensor = tract_ndarray::Array2::from_shape_vec((1, n), ids.to_vec())?.into();
        let mask_t: Tensor = tract_ndarray::Array2::from_shape_vec((1, n), vec![1i64; n])?.into();
        let us_t: Tensor = tract_ndarray::Array1::from_vec(vec![ustart as i64]).into();
        let out = self.model.run(tvec!(ids_t.into(), mask_t.into(), us_t.into()))?;
        Ok(out[0].to_plain_array_view::<f32>()?.iter().copied().collect())
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

    pub fn understand(&self, ctx: &Context, text: &str) -> TractResult<Understanding> {
        let (ids, us) = self.encode(ctx, text);
        Ok(self.decode(&self.logits(&ids, us)?))
    }
}

impl Classifier for Brain {
    /// The likeliest acts for a sentence said with no earlier context. Follow-ups ("and tomorrow?")
    /// need [`Brain::understand`] with a [`Context`]; the engine does not pass one yet.
    fn classify(&self, text: &str, _lang: &str) -> Vec<Prediction> {
        match self.understand(&Context::empty(), text) {
            Ok(u) => u.acts.into_iter().map(|s| Prediction { label: s.label, p: s.p }).collect(),
            Err(_) => Vec::new(),
        }
    }
}
