mod common;
use std::process::{Command, Output};
use weksa::{InventoryStore, PacketStore};

fn weksa(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_weksa"))
        .current_dir(dir)
        .args(["--corpus", "./c.cc", "--inventory", "./i.cc"])
        .args(args)
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

const ADD: [&str; 14] = [
    "corpus",
    "add",
    "--slug",
    "nibu",
    "--text",
    "Mind the hatch.",
    "--speaker",
    "nibu",
    "--group",
    "scene-1",
    "--admitted-on",
    "2026-10-29",
    "--source",
    "cast-authored",
];

#[test]
fn seed_add_dump_split_withdraw() {
    let d = tempfile::tempdir().unwrap();
    let seeded = weksa(d.path(), &["inventory", "seed"]);
    assert!(seeded.status.success(), "{}", out(&seeded));
    assert!(out(&seeded).contains("seeded rev:1"));
    let again = weksa(d.path(), &["inventory", "seed"]);
    assert!(
        !again.status.success(),
        "a second seed is refused: {}",
        out(&again)
    );
    let dump = out(&weksa(d.path(), &["inventory", "dump"]));
    assert!(
        dump.contains("rev:1") && dump.contains("speech_act:5") && dump.contains("warn"),
        "{dump}"
    );

    // A cast-authored item needs its persona and fixture.
    let missing = weksa(d.path(), &ADD);
    assert!(
        !missing.status.success() && out(&missing).contains("--persona"),
        "{}",
        out(&missing)
    );
    let mut add = ADD.to_vec();
    add.extend(["--persona", "nibu", "--fixture", "f.yaml"]);
    let added = weksa(d.path(), &add);
    assert!(
        added.status.success() && out(&added).contains("Added item:nibu:1"),
        "{}",
        out(&added)
    );
    let second = weksa(d.path(), &add);
    assert!(
        out(&second).contains("Added item:nibu:2"),
        "{}",
        out(&second)
    );

    let dump = out(&weksa(d.path(), &["corpus", "dump"]));
    assert!(
        dump.contains("item:nibu:1")
            && dump.contains("Mind the hatch.")
            && dump.contains("CastAuthored"),
        "{dump}"
    );
    let split = out(&weksa(d.path(), &["split", "item:nibu:1"]));
    assert!(
        ["Train", "Validation", "Test"]
            .iter()
            .any(|s| split.trim() == *s),
        "{split}"
    );
    assert!(!weksa(d.path(), &["split", "item:nibu:9"]).status.success());

    let withdrawn = weksa(d.path(), &["corpus", "withdraw", "item:nibu:1"]);
    assert!(withdrawn.status.success());
    assert!(out(&weksa(d.path(), &["corpus", "dump"])).contains("withdrawn"));
}

#[test]
fn model_written_and_public_domain_sources_name_their_provenance() {
    let d = tempfile::tempdir().unwrap();
    let base = [
        "corpus",
        "add",
        "--slug",
        "x",
        "--text",
        "t",
        "--speaker",
        "s",
        "--group",
        "g",
        "--admitted-on",
        "2026-10-29",
    ];
    let run = |extra: &[&str]| {
        let mut args = base.to_vec();
        args.extend_from_slice(extra);
        weksa(d.path(), &args)
    };
    let no_family = run(&["--source", "model-written"]);
    assert!(
        !no_family.status.success() && out(&no_family).contains("--model-family"),
        "{}",
        out(&no_family)
    );
    let model = run(&["--source", "model-written", "--model-family", "m"]);
    assert!(model.status.success(), "{}", out(&model));
    let no_work = run(&["--source", "public-domain", "--author", "a"]);
    assert!(
        !no_work.status.success() && out(&no_work).contains("--work"),
        "{}",
        out(&no_work)
    );
    let pd = run(&[
        "--source",
        "public-domain",
        "--work",
        "w",
        "--author",
        "a",
        "--edition-url",
        "https://example.org/w",
    ]);
    assert!(pd.status.success(), "{}", out(&pd));
    let bad_turn = run(&[
        "--source",
        "operator-written",
        "--prior-turn",
        "no equals sign",
    ]);
    assert!(
        !bad_turn.status.success() && out(&bad_turn).contains("speaker=text"),
        "{}",
        out(&bad_turn)
    );
    let turn = run(&[
        "--source",
        "operator-written",
        "--prior-turn",
        "player=hello",
    ]);
    assert!(turn.status.success(), "{}", out(&turn));
}

#[test]
fn validate_reports_each_packet() {
    let d = tempfile::tempdir().unwrap();
    let w = common::world();
    let full = w.dir.path().join("i.cc");
    let packets = d.path().join("p.cc");
    let packet = common::good();
    PacketStore::open(&packets)
        .unwrap()
        .admit(packet.clone(), &w.inventory, &w.corpus)
        .unwrap();

    let validate = |inventory: &std::path::Path, extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_weksa"))
            .arg("--inventory")
            .arg(inventory)
            .arg("validate")
            .arg(&packets)
            .args(extra)
            .output()
            .unwrap()
    };
    let ok = validate(&full, &[]);
    assert!(
        ok.status.success() && out(&ok).contains(&format!("ok {}", packet.id)),
        "{}",
        out(&ok)
    );
    assert!(validate(&full, &[packet.id.as_str()]).status.success());
    assert!(!validate(&full, &["packet:missing"]).status.success());
    // Against an inventory that only reaches revision 1 the same packet is refused, by rule.
    let short = d.path().join("short.cc");
    let (rev1, entries1) = weksa::inventory::seed();
    InventoryStore::open(&short)
        .unwrap()
        .admit_revision(rev1, entries1)
        .unwrap();
    let refused = validate(&short, &[]);
    assert!(
        !refused.status.success() && out(&refused).contains("V10"),
        "{}",
        out(&refused)
    );
}

#[test]
fn schema_writes_the_published_files() {
    let d = tempfile::tempdir().unwrap();
    let o = weksa(d.path(), &["schema", "--out", "s"]);
    assert!(o.status.success(), "{}", out(&o));
    let mut names: Vec<String> = std::fs::read_dir(d.path().join("s"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "weksa.corpus_item.schema.json",
            "weksa.interlingua_packet.schema.json",
            "weksa.inventory_entry.schema.json",
            "weksa.inventory_revision.schema.json"
        ]
    );
}
