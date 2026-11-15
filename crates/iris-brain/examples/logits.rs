//! Logits for given token ids, one line per input: `cargo run --release -p iris-brain --example logits -- <brain dir> <ids.tsv>`.
//! Used to compare runtimes on exactly the same input.

use iris_brain::Brain;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let brain = Brain::load(Path::new(&a[1])).expect("load the network");
    for line in std::fs::read_to_string(&a[2]).unwrap().lines() {
        let (ids, us) = line.split_once('\t').unwrap();
        let ids: Vec<i64> = ids.split(' ').map(|x| x.parse().unwrap()).collect();
        let v = brain.logits(&ids, us.parse().unwrap()).unwrap();
        println!("{}", v.iter().map(|x| format!("{x:.6}")).collect::<Vec<_>>().join(" "));
    }
}
