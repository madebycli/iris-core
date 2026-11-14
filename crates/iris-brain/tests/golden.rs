//! The Rust network must give what the Kotlin one gives. The model files are not in the repository:
//! point `IRIS_BRAIN_DIR` at a directory holding brain.onnx, meta.txt and pieces.tsv
//! (in the Iris app: app/src/main/assets/brain). Without it the test says so and passes.

use iris_brain::{Brain, Context, Tokenizer};
use std::path::PathBuf;

#[test]
fn matches_the_kotlin_runtime() {
    let Ok(dir) = std::env::var("IRIS_BRAIN_DIR") else {
        eprintln!("IRIS_BRAIN_DIR is not set: golden test skipped");
        return;
    };
    let dir = PathBuf::from(dir);
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
        let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        let argmax = |v: &[f32]| v.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        assert_eq!(argmax(&got[..76]), argmax(&want[..76]), "act of {text:?}");
        assert!(worst < 0.5, "logits of {text:?} differ by {worst}");
        eprintln!("{text:?}: act {} worst logit difference {worst:.4}", brain.understand(&ctx, text).unwrap().acts[0].label);
        n += 1;
    }
    assert!(n >= 5, "golden file has {n} lines");
    let _ = tok;
}
