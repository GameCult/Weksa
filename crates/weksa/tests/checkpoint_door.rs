//! A line-based tripwire on the Checkpoint door, not a proof. The real defence
//! is that `Checkpoint::for_test` is `cfg(test)`, pinned by the compile_fail
//! doctest on its current name in `split.rs`. This scan reads `split.rs` line by
//! line and catches the common single-line forms: a `pub fn` in `impl
//! Checkpoint` whose signature line says `-> Self` or `-> Checkpoint` without
//! `cfg(test)` above it, a trait impl for `Checkpoint`, a derive on it, a `pub`
//! field in its braces, and a `-> Checkpoint` line in any other `src` file. It
//! misses: signatures wrapped over several lines, `Option`, `Box`, `Result` or
//! tuple returns from `split.rs`, tuple structs with `pub` fields, and
//! constructors made by macros.

/// Why `src` lets code outside the lib make a `Checkpoint`, if it does.
fn door_leaks(split_rs: &str) -> Vec<String> {
    let mut leaks = Vec::new();
    let lines: Vec<&str> = split_rs.lines().collect();
    let mut in_impl = false;
    let mut in_struct = false;
    for (n, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with("impl Checkpoint") {
            in_impl = true;
        } else if t.starts_with("impl") && t.contains("Checkpoint") {
            leaks.push(format!("{}: trait impl: {t}", n + 1));
        }
        if t.starts_with("pub struct Checkpoint") {
            let derived = lines[..n]
                .iter()
                .rev()
                .take_while(|l| l.trim().starts_with("#[") || l.trim().starts_with("///"))
                .any(|l| l.contains("derive"));
            if derived {
                leaks.push(format!("{}: derive on Checkpoint", n + 1));
            }
            in_struct = true;
            continue;
        }
        if in_struct {
            if t.starts_with('}') {
                in_struct = false;
            } else if t.starts_with("pub ") {
                leaks.push(format!("{}: public field: {t}", n + 1));
            }
        }
        if in_impl && t.starts_with("pub ") && t.contains("fn ") {
            let mints = t.contains("-> Self") || t.contains("-> Checkpoint");
            let test_only = lines[..n]
                .iter()
                .rev()
                .take_while(|l| l.trim().starts_with("#["))
                .any(|l| l.contains("cfg(test)"));
            if mints && !test_only {
                leaks.push(format!("{}: pub constructor: {t}", n + 1));
            }
        }
    }
    leaks
}

#[test]
fn no_public_item_mints_a_checkpoint() {
    let src =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/split.rs")).unwrap();
    assert_eq!(door_leaks(&src), Vec::<String>::new());
    // No other module returns one either.
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("split.rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            let minting = text
                .lines()
                .filter(|l| l.contains("-> Checkpoint") || l.contains("-> Result<Checkpoint"))
                .count();
            assert_eq!(minting, 0, "{}", path.display());
        }
    }
}

#[test]
fn the_scan_sees_each_way_to_open_the_door() {
    let good = "impl Checkpoint {\n    #[cfg(test)]\n    pub(crate) fn for_test(r: u32) -> Self { x }\n}\n";
    assert!(door_leaks(good).is_empty());
    // Test constructor made public, under its own name or another.
    let public = "impl Checkpoint {\n    pub fn for_test(r: u32) -> Self { x }\n}\n";
    assert_eq!(door_leaks(public).len(), 1);
    let renamed = "impl Checkpoint {\n    #[cfg(test)]\n    #[allow(x)]\n    pub fn new(r: u32) -> Checkpoint { x }\n}\n";
    assert!(
        door_leaks(renamed).is_empty(),
        "cfg(test) above another attribute counts"
    );
    let renamed = "impl Checkpoint {\n    pub fn new(r: u32) -> Checkpoint { x }\n}\n";
    assert_eq!(door_leaks(renamed).len(), 1);
    // cfg(test) on the previous item does not cover this one.
    let stray =
        "impl Checkpoint {\n    #[cfg(test)]\n    fn a() {}\n    pub fn b() -> Self { x }\n}\n";
    assert_eq!(door_leaks(stray).len(), 1);
    assert_eq!(door_leaks("impl Default for Checkpoint {}\n").len(), 1);
    assert_eq!(door_leaks("impl From<u32> for Checkpoint {}\n").len(), 1);
    assert_eq!(
        door_leaks("#[derive(Clone)]\npub struct Checkpoint {\n").len(),
        1
    );
    assert_eq!(
        door_leaks("pub struct Checkpoint {\n    pub round: u32,\n}\n").len(),
        1
    );
    assert!(door_leaks("pub struct Checkpoint {\n    round: u32,\n}\n").is_empty());
}
