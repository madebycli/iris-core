//! `iris "erinnere mich um fünf an den Hund"` answers once; without a sentence it reads one per line.
//! `iris spec <files>` runs conversation specs.

use iris_cli::{run_spec, Session};
use std::io::BufRead;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("spec") {
        let mut failed = 0;
        for path in &args[1..] {
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            let (checks, failures) = run_spec(&source);
            println!("{path}: {checks} checks, {} failed", failures.len());
            for f in &failures {
                println!("  line {} [{}] {}", f.line, f.case, f.message);
            }
            failed += failures.len();
        }
        std::process::exit(if failed == 0 { 0 } else { 1 });
    }
    let mut lang = "de".to_string();
    if let Some(i) = args.iter().position(|a| a == "--lang") {
        lang = args.get(i + 1).cloned().unwrap_or(lang);
        args.drain(i..(i + 2).min(args.len()));
    }
    let now = jiff::Zoned::now();
    let mut session = Session::new(now.timestamp().as_millisecond(), now.time_zone().iana_name().unwrap_or("UTC"), &lang);
    let host = session.host.clone();
    let mut answer = |text: &str| {
        let reply = session.say(text);
        println!("[{}/{} {:?}] {}", reply.skill, reply.intent, reply.kind, reply.text);
    };
    if args.is_empty() {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            // real time passes between lines
            host.0.lock().unwrap().now_ms = jiff::Timestamp::now().as_millisecond();
            answer(&line);
        }
    } else {
        answer(&args.join(" "));
    }
}
