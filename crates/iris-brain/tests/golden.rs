//! The network must give the answers it gave when it was exported. The model is `models/brain` in this repository;
//! `IRIS_BRAIN_DIR` points at another directory with brain.onnx, meta.txt and pieces.tsv. Without a model the test says so and passes.

use iris_brain::{Brain, Context, Tokenizer};
use std::path::PathBuf;

#[test]
fn matches_the_kotlin_runtime() {
    let dir = match std::env::var("IRIS_BRAIN_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../models/brain")),
    };
    if !dir.join("brain.onnx").exists() {
        eprintln!("no network in {}: golden test skipped", dir.display());
        return;
    }
    let tok = Tokenizer::parse(&std::fs::read_to_string(dir.join("pieces.tsv")).unwrap());
    let brain = Brain::load(&dir).unwrap();
    let golden = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.tsv")).unwrap();
    let mut n = 0;
    for line in golden.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let (ctx, text) = (Context::parse(f[0]), f[1]);
        let want_ids: Vec<i64> = f[2].split(' ').map(|x| x.parse().unwrap()).collect();
        let want_us: usize = f[3].parse().unwrap();
        let want: Vec<f32> = f[4].split(' ').map(|x| x.parse().unwrap()).collect();
        let (ids, us) = brain.encode(&ctx, text);
        assert_eq!(ids, want_ids, "ids of {text:?}");
        assert_eq!(us, want_us, "utterance start of {text:?}");
        let got = brain.logits(&ids, us).unwrap();
        assert_eq!(got.len(), want.len(), "output size of {text:?}");
        // The golden values come from ONNX Runtime 1.31. Its int8 kernels round differently from 1.28 (the
        // version the app ships): up to about 0.6 in single logits on x86, which can tip a head that was a
        // coin flip at export. What must not change is a clear answer (p >= 0.6 at export): the act and
        // every head the assistant reads.
        let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        let (g, w) = (brain.decode(&got), brain.decode(&want));
        assert_eq!(g.acts[0].label, w.acts[0].label, "act of {text:?}");
        if (w.acts[0].p - 0.6).abs() > 0.05 {
            assert_eq!(g.acts[0].p >= 0.6, w.acts[0].p >= 0.6, "confidence of the act of {text:?}: {} vs {}", g.acts[0].p, w.acts[0].p);
        }
        for ((head, gs), (_, ws)) in g.heads.iter().zip(&w.heads) {
            if ws.p >= 0.6 {
                assert_eq!(gs.label, ws.label, "head {head} of {text:?}: {gs:?} vs {ws:?}");
            }
        }
        assert!(worst < 1.0, "logits of {text:?} differ by {worst}");
        eprintln!("{text:?}: act {} worst logit difference {worst:.4}", brain.understand(&ctx, text).unwrap().acts[0].label);
        n += 1;
    }
    assert!(n >= 5, "golden file has {n} lines");
    let _ = tok;
}
