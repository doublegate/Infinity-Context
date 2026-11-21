#[path = "/home/parobek/Code/OSS_Public-Projects/Infinity-Context/crates/icx-codec/src/redact.rs"]
mod redact;
use redact::SecretRedactor;

fn fails(s: &str) -> bool {
    let once = SecretRedactor::redact(s);
    SecretRedactor::redact(&once) != once
}

fn main() {
    let input = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    assert!(fails(&input));
    println!("original={input:?}");
    let once = SecretRedactor::redact(&input);
    println!("once={once:?}\ntwice={:?}", SecretRedactor::redact(&once));
    let mut reduced = input;
    let mut block = reduced.len() / 2;
    while block > 0 {
        let mut at = 0;
        while at + block <= reduced.len() {
            if !reduced.is_char_boundary(at) || !reduced.is_char_boundary(at+block) {
                at += 1;
                continue;
            }
            let mut candidate = reduced.clone();
            candidate.replace_range(at..at+block, "");
            if fails(&candidate) { reduced = candidate; at = 0; } else { at += 1; }
        }
        block /= 2;
    }
    let once = SecretRedactor::redact(&reduced);
    println!("reduced={reduced:?}\nreduced_once={once:?}\nreduced_twice={:?}", SecretRedactor::redact(&once));
}
