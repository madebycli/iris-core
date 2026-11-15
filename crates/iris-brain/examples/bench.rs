//! Speed of the network: `cargo run --release -p iris-brain --example bench -- <brain dir> <sentences> <ids out> [rounds]`.
//! Writes the token ids of every sentence, so another runtime can be timed on exactly the same input,
//! and prints wall time and CPU time (the battery's share) per inference.

use iris_brain::{Brain, Context};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

fn cpu_seconds() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime only writes into the timespec we pass.
    unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (dir, sentences, ids_out) = (Path::new(&a[1]), &a[2], &a[3]);
    let rounds: usize = a.get(4).map(|r| r.parse().unwrap()).unwrap_or(5);

    let (w0, c0) = (Instant::now(), cpu_seconds());
    let brain = Brain::load(dir).expect("load the network");
    println!("load: wall {:.1} ms, cpu {:.1} ms", w0.elapsed().as_secs_f64() * 1e3, (cpu_seconds() - c0) * 1e3);

    let ctx = Context::default();
    let inputs: Vec<(Vec<i64>, usize)> =
        std::fs::read_to_string(sentences).unwrap().lines().filter(|l| !l.trim().is_empty()).map(|s| brain.encode(&ctx, s)).collect();
    let mut f = std::fs::File::create(ids_out).unwrap();
    for (ids, us) in &inputs {
        writeln!(f, "{}\t{}", ids.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "), us).unwrap();
    }

    // the first call builds caches: not part of a steady answer
    let (w0, c0) = (Instant::now(), cpu_seconds());
    brain.logits(&inputs[0].0, inputs[0].1).unwrap();
    println!("first inference: wall {:.2} ms, cpu {:.2} ms", w0.elapsed().as_secs_f64() * 1e3, (cpu_seconds() - c0) * 1e3);
    for (ids, us) in inputs.iter().take(20) {
        brain.logits(ids, *us).unwrap();
    }

    let mut walls = Vec::new();
    let c0 = cpu_seconds();
    for _ in 0..rounds {
        for (ids, us) in &inputs {
            let t = Instant::now();
            brain.logits(ids, *us).unwrap();
            walls.push(t.elapsed().as_secs_f64() * 1e3);
        }
    }
    let cpu = (cpu_seconds() - c0) * 1e3 / walls.len() as f64;
    walls.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = walls.iter().sum::<f64>() / walls.len() as f64;
    let pct = |p: f64| walls[((walls.len() - 1) as f64 * p) as usize];
    println!(
        "onnxruntime: {} inferences, wall mean {:.2} ms, p50 {:.2}, p90 {:.2}, p99 {:.2}; cpu mean {:.2} ms",
        walls.len(), mean, pct(0.5), pct(0.9), pct(0.99), cpu
    );
}
