mod common;
use std::process::{Command, Output};
use weksa::{CorpusStore, EntityStore, InventoryStore, PacketStore};

fn weksa(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_weksa"))
        .current_dir(dir)
        .args([
            "--corpus",
            "./c.cc",
            "--inventory",
            "./i.cc",
            "--entities",
            "./e.cc",
        ])
        .args(args)
        .output()
        .unwrap()
}

/// Admits the cast entities the tests cite.
fn cast_entities(dir: &std::path::Path, slugs: &[&str]) {
    for slug in slugs {
        let id = format!("entity:{slug}");
        let form = format!("en-US={}", slug.to_uppercase());
        let put = weksa(
            dir,
            &[
                "entity",
                "put",
                &id,
                "--form",
                &form,
                "--origin",
                "cast",
                "--persona",
                slug,
                "--fixture",
                "f.yaml",
            ],
        );
        assert!(put.status.success(), "{}", out(&put));
    }
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
    "entity:nibu",
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
    cast_entities(d.path(), &["nibu"]);
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

    // A cast-authored item needs its persona, fixture and author.
    let missing = weksa(d.path(), &ADD);
    assert!(
        !missing.status.success() && out(&missing).contains("--persona"),
        "{}",
        out(&missing)
    );
    let mut add = ADD.to_vec();
    add.extend([
        "--persona",
        "nibu",
        "--fixture",
        "f.yaml",
        "--author",
        "Ada Writer",
    ]);
    let added = weksa(d.path(), &add);
    assert!(
        added.status.success() && out(&added).contains("Added item:nibu:1"),
        "{}",
        out(&added)
    );
    // The same text under a new key is refused (S6); another text is not.
    let copy = weksa(d.path(), &add);
    assert!(
        !copy.status.success() && out(&copy).contains("S6TextHeld"),
        "{}",
        out(&copy)
    );
    let mut other = add.clone();
    other[5] = "Mind the other hatch.";
    let second = weksa(d.path(), &other);
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
    cast_entities(d.path(), &["s", "player"]);
    let base = [
        "corpus",
        "add",
        "--slug",
        "x",
        "--speaker",
        "entity:s",
        "--group",
        "g",
        "--admitted-on",
        "2026-10-29",
    ];
    // S6 refuses one text under two keys, so each run writes its own.
    let runs = std::cell::Cell::new(0);
    let run = |extra: &[&str]| {
        runs.set(runs.get() + 1);
        let text = format!("line {}", runs.get());
        let mut args = base.to_vec();
        args.extend_from_slice(&["--text", &text]);
        args.extend_from_slice(extra);
        weksa(d.path(), &args)
    };
    let no_family = run(&["--source", "model-written"]);
    assert!(
        !no_family.status.success() && out(&no_family).contains("--model-family"),
        "{}",
        out(&no_family)
    );
    let not_an_enum_value = run(&["--source", "model-written", "--model-family", "m"]);
    assert!(
        !not_an_enum_value.status.success(),
        "{}",
        out(&not_an_enum_value)
    );
    let model = run(&[
        "--source",
        "model-written",
        "--model-family",
        "open-ai",
        "--generator",
        "g",
    ]);
    assert!(model.status.success(), "{}", out(&model));
    let dump = weksa(d.path(), &["corpus", "dump"]);
    assert!(
        out(&dump).contains("OpenAi") && out(&dump).contains("generator: Some(\"g\")"),
        "{}",
        out(&dump)
    );
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
        "--licence",
        "Public domain",
        "--jurisdiction",
        "US",
        "--translator",
        "T. Translator",
        "--edition-year",
        "1891",
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
        "entity:player=hello",
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
        .admit(packet.clone(), &w.inventory, &w.corpus, &w.entities)
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
        !refused.status.success()
            && out(&refused).contains("V10: the inventory revision has no record"),
        "{}",
        out(&refused)
    );
    assert!(
        !out(&refused).contains("V10UnknownRevision"),
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
            "weksa.entity.schema.json",
            "weksa.interlingua_packet.schema.json",
            "weksa.inventory_entry.schema.json",
            "weksa.inventory_revision.schema.json"
        ]
    );
}

#[test]
fn entity_put_and_name_spans() {
    let d = tempfile::tempdir().unwrap();
    let put = |args: &[&str]| {
        let mut all = vec![
            "entity",
            "put",
            "entity:kay",
            "--origin",
            "cast",
            "--fixture",
            "f.yaml",
        ];
        all.extend_from_slice(args);
        weksa(d.path(), &all)
    };
    let no_persona = put(&["--form", "en-US=Kay"]);
    assert!(
        !no_persona.status.success() && out(&no_persona).contains("--persona"),
        "{}",
        out(&no_persona)
    );
    let first = put(&["--form", "en-US=Kay", "--persona", "kay"]);
    assert!(
        first.status.success() && out(&first).contains("Added entity:kay"),
        "{}",
        out(&first)
    );
    // Forms append; a language's forms keep the order given.
    let grown = put(&[
        "--form",
        "en-US=Kay",
        "--form",
        "en-US=Kay's",
        "--persona",
        "kay",
    ]);
    assert!(grown.status.success(), "{}", out(&grown));
    let shrunk = put(&["--form", "en-US=Kay", "--persona", "kay"]);
    assert!(
        !shrunk.status.success() && out(&shrunk).contains("EntityNotAppendOnly"),
        "{}",
        out(&shrunk)
    );
    let dump = out(&weksa(d.path(), &["entity", "dump"]));
    assert!(
        dump.contains("entity:kay") && dump.contains("Kay's"),
        "{dump}"
    );

    cast_entities(d.path(), &["nibu"]);
    let add = |range: &str, text: &str| {
        weksa(
            d.path(),
            &[
                "corpus",
                "add",
                "--slug",
                "kay",
                "--text",
                text,
                "--speaker",
                "entity:nibu",
                "--group",
                "g",
                "--admitted-on",
                "2026-10-29",
                "--source",
                "operator-written",
                "--name",
                range,
            ],
        )
    };
    let named = add("0..3=entity:kay", "Kay left.");
    assert!(named.status.success(), "{}", out(&named));
    let unequal = add("0..3=entity:kay", "Kai left.");
    assert!(
        !unequal.status.success() && out(&unequal).contains("NameSpanNotAForm"),
        "{}",
        out(&unequal)
    );
    let malformed = add("0-3", "Kay again.");
    assert!(
        !malformed.status.success() && out(&malformed).contains("start..end=entity"),
        "{}",
        out(&malformed)
    );
    let dump = out(&weksa(d.path(), &["corpus", "dump"]));
    assert!(
        dump.contains("names:") && dump.contains("entity:kay"),
        "{dump}"
    );
}

#[test]
fn entity_origin_flags_must_match_origin() {
    let d = tempfile::tempdir().unwrap();
    let put = |args: &[&str]| {
        let mut all = vec!["entity", "put", "entity:kay", "--form", "en-US=Kay"];
        all.extend_from_slice(args);
        weksa(d.path(), &all)
    };
    for mismatched in [
        &["--origin", "live"][..],
        &["--origin", "live", "--persona", "kay", "--work", "edda"],
        &[
            "--origin",
            "live",
            "--persona",
            "kay",
            "--fixture",
            "f.yaml",
        ],
        &["--origin", "cast", "--persona", "kay"],
        &["--origin", "cast", "--fixture", "f.yaml"],
        &[
            "--origin",
            "cast",
            "--persona",
            "kay",
            "--fixture",
            "f.yaml",
            "--work",
            "edda",
        ],
        &["--origin", "public-domain"],
        &[
            "--origin",
            "public-domain",
            "--work",
            "edda",
            "--persona",
            "kay",
        ],
        &[
            "--origin",
            "public-domain",
            "--work",
            "edda",
            "--fixture",
            "f.yaml",
        ],
        &["--origin", "cast", "--persona", "", "--fixture", "f.yaml"],
        &["--origin", "cast", "--persona", "kay", "--fixture", ""],
        &["--origin", "live", "--persona", ""],
        &["--origin", "public-domain", "--work", ""],
    ] {
        let o = put(mismatched);
        assert!(!o.status.success(), "{mismatched:?}: {}", out(&o));
    }
    assert!(!out(&weksa(d.path(), &["entity", "dump"])).contains("entity:kay"));
    assert!(
        put(&["--origin", "live", "--persona", "kay"])
            .status
            .success()
    );
    let work = weksa(
        d.path(),
        &[
            "entity",
            "put",
            "entity:brokkr",
            "--form",
            "en-US=Brokkr",
            "--origin",
            "public-domain",
            "--work",
            "edda",
        ],
    );
    assert!(work.status.success(), "{}", out(&work));
    assert!(out(&weksa(d.path(), &["entity", "dump"])).contains("PublicDomain"));
}

#[test]
fn intake_normalizes_to_nfc() {
    let d = tempfile::tempdir().unwrap();
    // A decomposed form and decomposed text meet as the same composed bytes.
    let put = weksa(
        d.path(),
        &[
            "entity",
            "put",
            "entity:rene",
            "--form",
            "fr=Rene\u{301}",
            "--origin",
            "live",
            "--persona",
            "rene",
        ],
    );
    assert!(put.status.success(), "{}", out(&put));
    cast_entities(d.path(), &["nibu"]);
    let add = weksa(
        d.path(),
        &[
            "corpus",
            "add",
            "--slug",
            "rene",
            "--text",
            "Rene\u{301} part.",
            "--language",
            "fr",
            "--speaker",
            "entity:nibu",
            "--group",
            "g",
            "--admitted-on",
            "2026-10-29",
            "--source",
            "operator-written",
            "--name",
            "0..5=entity:rene",
        ],
    );
    assert!(add.status.success(), "{}", out(&add));
    let dump = out(&weksa(d.path(), &["corpus", "dump"]));
    assert!(dump.contains("Ren\u{e9} part."), "{dump}");
}

/// `corpus add` of "Mind the hatch." by `entity:nibu` under `source`, with
/// `extra` flags.
fn add_with(dir: &std::path::Path, source: &str, extra: &[&str]) -> Output {
    let mut args = vec![
        "corpus",
        "add",
        "--slug",
        "nibu",
        "--text",
        "Mind the hatch.",
        "--speaker",
        "entity:nibu",
        "--group",
        "scene-1",
        "--admitted-on",
        "2026-10-29",
        "--source",
        source,
    ];
    args.extend_from_slice(extra);
    weksa(dir, &args)
}

#[test]
fn source_flags_must_match_the_source() {
    let d = tempfile::tempdir().unwrap();
    cast_entities(d.path(), &["nibu"]);
    // A provenance flag that does not apply to --source refuses, naming the flag.
    for (source, extra, flag) in [
        ("operator-written", &["--persona", "p"][..], "--persona"),
        (
            "operator-approved",
            &["--note", "n", "--work", "w"],
            "--work",
        ),
        (
            "public-domain",
            &[
                "--work",
                "w",
                "--author",
                "a",
                "--edition-url",
                "u",
                "--licence",
                "l",
                "--jurisdiction",
                "j",
                "--translator",
                "t",
                "--edition-year",
                "1891",
                "--note",
                "n",
            ],
            "--note",
        ),
        (
            "cast-authored",
            &[
                "--persona",
                "p",
                "--fixture",
                "f",
                "--author",
                "a",
                "--generator",
                "g",
            ],
            "--generator",
        ),
        (
            "model-written",
            &["--model-family", "unrecorded", "--edition-url", "u"],
            "--edition-url",
        ),
        (
            "operator-written",
            &["--model-family", "unrecorded"],
            "--model-family",
        ),
        (
            "model-written",
            &[
                "--model-family",
                "unrecorded",
                "--prompt-digest",
                "d",
                "--fixture",
                "f",
            ],
            "--fixture",
        ),
        // A source missing a flag it needs.
        (
            "cast-authored",
            &["--persona", "p", "--fixture", "f"],
            "--author",
        ),
        (
            "public-domain",
            &["--work", "w", "--edition-url", "u"],
            "--author",
        ),
        (
            "public-domain",
            &[
                "--work",
                "w",
                "--author",
                "a",
                "--edition-url",
                "u",
                "--jurisdiction",
                "j",
                "--translator",
                "t",
                "--edition-year",
                "1891",
            ],
            "--licence",
        ),
        (
            "public-domain",
            &[
                "--work",
                "w",
                "--author",
                "a",
                "--edition-url",
                "u",
                "--licence",
                "l",
                "--translator",
                "t",
                "--edition-year",
                "1891",
            ],
            "--jurisdiction",
        ),
        (
            "public-domain",
            &[
                "--work",
                "w",
                "--author",
                "a",
                "--edition-url",
                "u",
                "--licence",
                "l",
                "--jurisdiction",
                "j",
                "--edition-year",
                "1891",
            ],
            "--translator",
        ),
        (
            "public-domain",
            &[
                "--work",
                "w",
                "--author",
                "a",
                "--edition-url",
                "u",
                "--licence",
                "l",
                "--jurisdiction",
                "j",
                "--translator",
                "t",
            ],
            "--edition-year",
        ),
        // Empty provenance strings.
        (
            "model-written",
            &["--model-family", "unrecorded", "--generator", ""],
            "",
        ),
        (
            "model-written",
            &["--model-family", "unrecorded", "--prompt-digest", ""],
            "",
        ),
        (
            "cast-authored",
            &["--persona", "", "--fixture", "f", "--author", "a"],
            "",
        ),
        ("operator-approved", &["--note", ""], ""),
    ] {
        let o = add_with(d.path(), source, extra);
        assert!(!o.status.success(), "{source} {extra:?}: {}", out(&o));
        assert!(out(&o).contains(flag), "{source} {extra:?}: {}", out(&o));
    }
    assert!(!out(&weksa(d.path(), &["corpus", "dump"])).contains("item:"));
    // A source whose flags all apply is admitted; --author reaches a
    // cast-authored item.
    let cast = add_with(
        d.path(),
        "cast-authored",
        &["--persona", "p", "--fixture", "f", "--author", "Ada Writer"],
    );
    assert!(cast.status.success(), "{}", out(&cast));
    let dump = out(&weksa(d.path(), &["corpus", "dump"]));
    assert!(dump.contains("author: \"Ada Writer\""), "{dump}");
}

#[test]
fn malformed_intake_never_echoes_its_values() {
    const CANARY: &str = "CANARY-7f3a";
    let d = tempfile::tempdir().unwrap();
    cast_entities(d.path(), &["nibu"]);
    let shape = |value: &str| format!("{CANARY}{value}");
    let puts = [
        (vec![shape("")], "--form"),
        (vec![shape("_x=Kay")], ""),
        (vec![format!("en-US={CANARY}\u{7}")], ""),
        (vec![format!("{CANARY}_x=Kay")], ""),
    ];
    for (forms, flag) in puts {
        let mut args = vec![
            "entity",
            "put",
            "entity:kay",
            "--origin",
            "live",
            "--persona",
            "kay",
        ];
        for f in &forms {
            args.extend(["--form", f]);
        }
        let o = weksa(d.path(), &args);
        assert!(!o.status.success(), "{forms:?}: {}", out(&o));
        assert!(!out(&o).contains(CANARY), "{forms:?}: {}", out(&o));
        assert!(out(&o).contains(flag), "{forms:?}: {}", out(&o));
    }
    let bad_turn = [shape(""), format!("{CANARY}=hello")];
    let bad_name = [
        shape(""),
        format!("0..1={CANARY}"),
        format!("{CANARY}..1=entity:nibu"),
        format!("0..{CANARY}=entity:nibu"),
    ];
    let cases = bad_turn
        .iter()
        .map(|t| ("--prior-turn", t))
        .chain(bad_name.iter().map(|n| ("--name", n)));
    for (flag, value) in cases {
        let o = add_with(d.path(), "operator-written", &[flag, value]);
        assert!(!o.status.success(), "{flag} {value}: {}", out(&o));
        assert!(!out(&o).contains(CANARY), "{flag} {value}: {}", out(&o));
        assert!(out(&o).contains(flag), "{flag} {value}: {}", out(&o));
    }
}

#[test]
fn every_decomposed_form_and_turn_is_stored_composed() {
    let d = tempfile::tempdir().unwrap();
    cast_entities(d.path(), &["nibu"]);
    let put = weksa(
        d.path(),
        &[
            "entity",
            "put",
            "entity:rene",
            "--form",
            "fr=Rene\u{301}",
            "--form",
            "fr=Zoe\u{308}",
            "--origin",
            "live",
            "--persona",
            "rene",
        ],
    );
    assert!(put.status.success(), "{}", out(&put));
    let rene = EntityStore::open(&d.path().join("e.cc"))
        .unwrap()
        .get(&"entity:rene".parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(rene.forms[0].forms, ["Ren\u{e9}", "Zo\u{eb}"]);
    let add = add_with(
        d.path(),
        "operator-written",
        &["--prior-turn", "entity:rene=Cafe\u{301} now"],
    );
    assert!(add.status.success(), "{}", out(&add));
    let item = CorpusStore::open(&d.path().join("c.cc"))
        .unwrap()
        .get("item:nibu:1")
        .unwrap()
        .unwrap();
    assert_eq!(item.prior_turns[0].text, "Caf\u{e9} now");
}

/// An Ollama stand-in: every text embeds to `[1, its length]`, so identical
/// texts have cosine 1.
fn fake_embedder() -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = vec![0u8; 65536];
            let mut n = 0;
            let body = loop {
                n += stream.read(&mut buf[n..]).unwrap();
                let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let want: usize = head
                        .to_lowercase()
                        .split("content-length: ")
                        .nth(1)
                        .unwrap()
                        .split("\r\n")
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap();
                    if body.len() >= want {
                        break body.to_string();
                    }
                }
            };
            let body: serde_json::Value = serde_json::from_str(&body).unwrap();
            let vectors: Vec<Vec<f64>> = body["input"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| vec![1.0, t.as_str().unwrap().len() as f64])
                .collect();
            let reply = serde_json::json!({ "embeddings": vectors }).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
        }
    });
    url
}

#[test]
fn proof_packet_lowers_and_the_lowering_scores_against_items() {
    let d = tempfile::tempdir().unwrap();
    cast_entities(d.path(), &["nibu"]);
    for args in [&["inventory", "seed"][..], &["inventory", "proof"]] {
        let run = weksa(d.path(), args);
        assert!(run.status.success(), "{}", out(&run));
    }
    let item = weksa(
        d.path(),
        &[
            "corpus",
            "add",
            "--slug",
            "nibu",
            "--text",
            "Warning: you may endanger yourself by touching the control. NIBU will stop that.",
            "--speaker",
            "entity:nibu",
            "--group",
            "scene-1",
            "--admitted-on",
            "2026-11-01",
            "--source",
            "operator-written",
        ],
    );
    assert!(item.status.success(), "{}", out(&item));
    let admitted = weksa(d.path(), &["packet", "proof", "./p.cc"]);
    assert!(
        admitted.status.success() && out(&admitted).contains("admitted packet:6e6962752d303031"),
        "{}",
        out(&admitted)
    );
    let lowered = weksa(d.path(), &["lower", "./p.cc", "packet:6e6962752d303031"]);
    assert!(lowered.status.success(), "{}", out(&lowered));
    let text = String::from_utf8(lowered.stdout).unwrap();
    assert!(
        text.starts_with("Warning: you may endanger yourself"),
        "{text}"
    );
    // The referent cites entity:nibu: its stored en-US form (NIBU here) reaches the text as stored.
    assert!(text.contains("NIBU will stop"), "{text}");

    let missing = weksa(d.path(), &["lower", "./p.cc", "packet:0000000000000000"]);
    assert!(!missing.status.success() && out(&missing).contains("no such packet"));

    // IPA is the ruled pronunciation or nothing: spelling is not one.
    let args = ["lower", "./p.cc", "packet:6e6962752d303031", "--ipa"];
    let unruled = weksa(d.path(), &args);
    assert!(
        !unruled.status.success() && out(&unruled).contains("entity has no ruled pronunciation"),
        "{}",
        out(&unruled)
    );
    for leak in ["NoPronunciation", "entity:nibu", "entity {"] {
        assert!(!out(&unruled).contains(leak), "{leak}: {}", out(&unruled));
    }
    let ruled = weksa(
        d.path(),
        &[
            "entity",
            "put",
            "entity:nibu",
            "--form",
            "en-US=NIBU",
            "--origin",
            "cast",
            "--persona",
            "nibu",
            "--fixture",
            "f.yaml",
            "--pronunciation",
            "en-US=ˈnibu",
            "--ruled-by",
            "Nibu",
            "--ruled-on",
            "2026-11-07",
            "--ruling-source",
            "test",
            "--rejected",
            "ˈnɪbu",
        ],
    );
    assert!(ruled.status.success(), "{}", out(&ruled));
    let ipa = weksa(d.path(), &args);
    assert!(ipa.status.success(), "{}", out(&ipa));
    assert_eq!(String::from_utf8(ipa.stdout).unwrap(), "ˈnibu\n");
    // The phones line is the committed fixture, byte for byte, and is a different
    // output from --ipa, so the two flags cannot be combined.
    let phones = weksa(
        d.path(),
        &["lower", "./p.cc", "packet:6e6962752d303031", "--phones"],
    );
    assert!(phones.status.success(), "{}", out(&phones));
    let fixture = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/phonology/fixtures/nibu-en-US.phones.json"),
    )
    .unwrap();
    assert_eq!(String::from_utf8(phones.stdout).unwrap(), fixture);
    let both = weksa(
        d.path(),
        &[
            "lower",
            "./p.cc",
            "packet:6e6962752d303031",
            "--phones",
            "--ipa",
        ],
    );
    assert!(!both.status.success(), "{}", out(&both));
    let again = weksa(d.path(), &["lower", "./p.cc", "packet:6e6962752d303031"]);
    assert_eq!(String::from_utf8(again.stdout).unwrap(), text);

    std::fs::write(d.path().join("out.txt"), &text).unwrap();
    let url = fake_embedder();
    let scored = weksa(
        d.path(),
        &[
            "score",
            "item:nibu:1",
            "--output",
            "out.txt",
            "--ollama-url",
            &url,
        ],
    );
    assert!(scored.status.success(), "{}", out(&scored));
    assert_eq!(
        out(&scored).trim(),
        "item:nibu:1 chrf=100.00 meaning=1.0000"
    );
    let unknown = weksa(
        d.path(),
        &[
            "score",
            "item:nibu:9",
            "--output",
            "out.txt",
            "--ollama-url",
            &url,
        ],
    );
    assert!(!unknown.status.success() && out(&unknown).contains("no such item"));
    assert!(!out(&unknown).contains("item:nibu:9"), "{}", out(&unknown));
}

/// Runs `args` and demands a refusal whose text holds `reason` and none of the
/// identifiers the caller supplied.
fn refuses_without_echo(d: &std::path::Path, args: &[&str], reason: &str) {
    let o = weksa(d, args);
    let text = out(&o);
    assert!(!o.status.success(), "{args:?}: {text}");
    assert!(text.contains(reason), "{args:?} lacks {reason:?}: {text}");
    for a in args.iter().filter(|a| a.to_lowercase().contains("canary")) {
        // An argument may carry the canary beside flag syntax (`--x=...`); the
        // identifier itself is the part from the canary on.
        let id = &a[a.to_lowercase().find("canary").unwrap()..];
        assert!(
            !text.to_lowercase().contains(&id.to_lowercase()),
            "{args:?} echoes {id:?}: {text}"
        );
    }
}

#[test]
fn cli_errors_never_echo_identifiers() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    cast_entities(p, &["nibu"]);
    let add = |extra: &[&str]| -> Vec<String> {
        ["corpus", "add", "--admitted-on", "2026-10-29"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(String::from)
            .collect()
    };
    let run = |args: Vec<String>, reason: &str| {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        refuses_without_echo(p, &refs, reason);
    };
    // The positional entity id, malformed and well formed but unheld.
    refuses_without_echo(
        p,
        &[
            "entity",
            "put",
            "canary-id",
            "--form",
            "en-US=X",
            "--origin",
            "live",
            "--persona",
            "k",
        ],
        "not a EntityId",
    );
    // --speaker: a malformed id, and a well formed one no entity holds.
    run(
        add(&[
            "--slug",
            "q",
            "--text",
            "Hello there.",
            "--speaker",
            "canary-speaker",
            "--group",
            "g",
            "--source",
            "operator-written",
        ]),
        "--speaker",
    );
    run(
        add(&[
            "--slug",
            "q",
            "--text",
            "Hello there.",
            "--speaker",
            "entity:canary",
            "--group",
            "g",
            "--source",
            "operator-written",
        ]),
        "UnknownEntity",
    );
    // clap enum values.
    run(
        add(&[
            "--slug",
            "q",
            "--text",
            "Hello there.",
            "--speaker",
            "entity:nibu",
            "--group",
            "g",
            "--source",
            "canary-source",
        ]),
        "--source",
    );
    refuses_without_echo(
        p,
        &[
            "entity",
            "put",
            "entity:kay",
            "--form",
            "en-US=X",
            "--origin",
            "canary-origin",
        ],
        "--origin",
    );
    run(
        add(&[
            "--slug",
            "q",
            "--text",
            "Hello there.",
            "--speaker",
            "entity:nibu",
            "--group",
            "g",
            "--source",
            "model-written",
            "--model-family",
            "canary-family",
        ]),
        "--model-family",
    );
    // Keys on withdraw, split, validate, lower and score.
    refuses_without_echo(p, &["corpus", "withdraw", "item:canary:7"], "no such item");
    refuses_without_echo(p, &["split", "item:canary:7"], "no such item");
    std::fs::write(p.join("out.txt"), "text").unwrap();
    refuses_without_echo(
        p,
        &["score", "item:canary:7", "--output", "out.txt"],
        "no such item",
    );
    let packets = weksa(p, &["inventory", "seed"]);
    assert!(packets.status.success(), "{}", out(&packets));
    let made = weksa(p, &["packet", "proof", "p.cc"]);
    let _ = made;
    refuses_without_echo(
        p,
        &["validate", "p.cc", "packet:canary:7"],
        "no such packet",
    );
    refuses_without_echo(p, &["lower", "p.cc", "packet:canary:7"], "no such packet");
    // S6 names no held key; S5 names no group.
    let held = weksa(
        p,
        &add(&[
            "--slug",
            "canarya",
            "--text",
            "Fresh words here.",
            "--speaker",
            "entity:nibu",
            "--group",
            "canary-first",
            "--source",
            "operator-written",
        ])
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
    );
    assert!(held.status.success(), "{}", out(&held));
    let copy = add(&[
        "--slug",
        "other",
        "--text",
        "Fresh words here.",
        "--speaker",
        "entity:nibu",
        "--group",
        "g",
        "--source",
        "operator-written",
    ]);
    let copy = weksa(p, &copy.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(
        !copy.status.success() && out(&copy).contains("S6TextHeld"),
        "{}",
        out(&copy)
    );
    assert!(!out(&copy).contains("canary"), "{}", out(&copy));
    let moved = add(&[
        "--slug",
        "other",
        "--text",
        "Corrected words here.",
        "--speaker",
        "entity:nibu",
        "--group",
        "canary-second",
        "--supersedes",
        "item:canarya:1",
        "--source",
        "operator-written",
    ]);
    let moved = weksa(p, &moved.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(
        !moved.status.success() && out(&moved).contains("S5SupersedeGroup"),
        "{}",
        out(&moved)
    );
    assert!(!out(&moved).contains("canary"), "{}", out(&moved));
}

#[test]
fn admitted_on_must_be_a_real_calendar_date() {
    let d = tempfile::tempdir().unwrap();
    cast_entities(d.path(), &["nibu"]);
    for bad in ["2026-02-30", "", "junk", "2026-13-01", "2026-1-1"] {
        let mut args = ADD.to_vec();
        let at = args.iter().position(|a| *a == "--admitted-on").unwrap();
        args[at + 1] = bad;
        let o = weksa(d.path(), &args);
        assert!(!o.status.success(), "{bad:?}: {}", out(&o));
        assert!(out(&o).contains("--admitted-on"), "{bad:?}: {}", out(&o));
    }
    assert!(!out(&weksa(d.path(), &["corpus", "dump"])).contains("item:"));
}

#[test]
fn clap_errors_never_echo_the_token() {
    let d = tempfile::tempdir().unwrap();
    for args in [
        &["--sentinelflagzz"][..],
        &["validate", "--sentinelflagzz"],
        &["sentinelsubzz"],
        &["corpus", "sentinelsubzz"],
        &["--inventory"],
    ] {
        let o = weksa(d.path(), args);
        let text = out(&o);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {text}");
        assert!(text.contains("error:"), "{args:?}: {text}");
        assert!(
            !text.to_lowercase().contains("sentinel"),
            "{args:?} echoes the token: {text}"
        );
    }
}

#[test]
fn pronunciation_language_is_never_echoed() {
    let d = tempfile::tempdir().unwrap();
    let o = weksa(
        d.path(),
        &[
            "entity",
            "put",
            "entity:nibu",
            "--form",
            "en-US=NIBU",
            "--origin",
            "cast",
            "--persona",
            "nibu",
            "--fixture",
            "f.yaml",
            "--pronunciation",
            "sentinel-LANG=ˈnibu",
            "--ruled-by",
            "N",
            "--ruled-on",
            "2026-11-07",
            "--ruling-source",
            "t",
        ],
    );
    let text = out(&o);
    assert!(
        !o.status.success() && text.contains("names a language that has no --form"),
        "{text}"
    );
    assert!(!text.to_lowercase().contains("sentinel"), "{text}");
}
