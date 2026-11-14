//! Pins found by mutation testing store.rs, inventory.rs and main.rs. Each test
//! states what the production code must do, in domain terms.
mod common;
use common::*;
use std::process::{Command, Output};
use weksa::packet::Grade;
use weksa::store::EntityBound;
use weksa::{
    Annotator, Condition, EntityStore, Layer, OntologyEntry, OntologyStore, Pronunciation, Refusal,
    Slot, StoreError, StoreRefusal, Test,
};

fn refusal<T: std::fmt::Debug>(r: Result<T, StoreError>) -> StoreRefusal {
    match r {
        Err(StoreError::Refused(r)) => r,
        other => panic!("expected a store refusal, got {other:?}"),
    }
}

fn kay_saying(p: Pronunciation) -> weksa::Entity {
    let mut e = ent("kay", "en-US", &["Kay"]);
    e.forms[0].pronunciation = Some(p);
    e
}

fn ruling(ipa: &str, rejected: Vec<String>) -> Pronunciation {
    Pronunciation {
        ipa: ipa.into(),
        ruled_by: "Kay".into(),
        date: "2026-11-07".into(),
        source: "test".into(),
        rejected,
    }
}

#[test]
fn a_pronunciation_with_a_control_or_invisible_format_character_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut store = EntityStore::open(&d.path().join("e.cc")).unwrap();
    // Either kind alone is refused: a control character, and a zero-width space.
    for ipa in ["ˈk\u{7}eɪ", "ˈk\u{200b}eɪ"] {
        assert!(
            matches!(
                store.put(kay_saying(ruling(ipa, vec![]))),
                Err(StoreError::Refused(StoreRefusal::Malformed { .. }))
            ),
            "{ipa:?}"
        );
    }
    // The same for a rejected pronunciation.
    for bad in ["ˈk\u{7}aɪ", "ˈk\u{200b}aɪ"] {
        assert!(
            matches!(
                store.put(kay_saying(ruling("ˈkeɪ", vec![bad.into()]))),
                Err(StoreError::Refused(StoreRefusal::Malformed { .. }))
            ),
            "{bad:?}"
        );
    }
    assert_eq!(store.get(&eid("kay")).unwrap(), None);
}

#[test]
fn a_pronunciation_keeps_at_most_eight_rejected_variants() {
    let d = tempfile::tempdir().unwrap();
    let mut store = EntityStore::open(&d.path().join("e.cc")).unwrap();
    let variants = |n: usize| -> Vec<String> { (0..n).map(|i| format!("ˈkaɪ{i}")).collect() };
    assert_eq!(
        refusal(store.put(kay_saying(ruling("ˈkeɪ", variants(9))))),
        StoreRefusal::EntityBounds {
            id: eid("kay"),
            bound: EntityBound::Forms
        }
    );
    store
        .put(kay_saying(ruling("ˈkeɪ", variants(8))))
        .expect("eight rejected variants are the bound, not beyond it");
}

#[test]
fn an_ontology_entry_read_at_an_unknown_revision_is_refused_with_one_refusal() {
    let w = world();
    let mut onto = OntologyStore::open(&w.dir.path().join("o2.cc")).unwrap();
    let entry = OntologyEntry {
        language: "en-US".into(),
        n: 1,
        inventory_rev: 99,
        layer: Layer::Discourse,
        when: vec![Condition {
            slot: Slot::Stance,
            feature: iv(ANGER),
            test: Test::AtLeast(Grade::new(200).unwrap()),
        }],
        choice: "WARM".into(),
        examples: vec![],
        annotator: Annotator::Operator,
    };
    match onto.admit(vec![entry], &w.inventory) {
        Err(StoreError::Invalid(r)) => assert_eq!(r, vec![Refusal::V10UnknownRevision { rev: 99 }]),
        other => panic!("{other:?}"),
    }
}

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
            "--ontology",
            "./o.cc",
            "--speaker-state",
            "./s.cc",
        ])
        .args(args)
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

fn ok(o: Output) -> String {
    assert!(o.status.success(), "{}", out(&o));
    out(&o)
}

/// A scene bed: entities nibu and brokkr, the inventory through the
/// speaker-scene revision (rev 3).
fn scene_bed() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for slug in ["nibu", "brokkr"] {
        let id = format!("entity:{slug}");
        let form = format!("en-US={}", slug.to_uppercase());
        ok(weksa(
            d.path(),
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
        ));
    }
    for step in ["seed", "proof", "speaker-scene"] {
        ok(weksa(d.path(), &["inventory", step]));
    }
    d
}

/// Adds a cast-authored line; returns the key it printed ("Added <key>").
fn add_line(d: &std::path::Path, slug: &str, speaker: &str, group: &str, text: &str) -> String {
    let speaker = format!("entity:{speaker}");
    let printed = ok(weksa(
        d,
        &[
            "corpus",
            "add",
            "--slug",
            slug,
            "--text",
            text,
            "--speaker",
            &speaker,
            "--group",
            group,
            "--admitted-on",
            "2026-10-29",
            "--source",
            "cast-authored",
            "--language",
            "en-US",
            "--persona",
            "nibu",
            "--fixture",
            "f.yaml",
            "--author",
            "Ada Writer",
        ],
    ));
    printed.split_whitespace().nth(1).unwrap().to_string()
}

fn scene_put(d: &std::path::Path, group: &str, to: &str) -> Output {
    weksa(
        d,
        &[
            "scene",
            "put",
            "--group",
            group,
            "--to",
            to,
            "--inventory-rev",
            "3",
            "--model-family",
            "anthropic",
            "--generator",
            "a scene annotator",
            "--set",
            "intimacy=200",
        ],
    )
}

#[test]
fn a_scene_group_holds_one_live_speaker_among_its_own_live_items() {
    let d = scene_bed();
    add_line(d.path(), "nibu", "nibu", "scene-1", "Mind the hatch.");
    // Another group's speaker and a withdrawn line of this group are not the
    // scene's speakers.
    add_line(d.path(), "brokkr", "brokkr", "scene-2", "Brokkr hums.");
    let gone = add_line(
        d.path(),
        "brokkr",
        "brokkr",
        "scene-1",
        "Brokkr leaves early.",
    );
    ok(weksa(d.path(), &["corpus", "withdraw", &gone]));
    let put = ok(scene_put(d.path(), "scene-1", "unnamed"));
    assert!(put.contains("scene:scene-1@1"), "{put}");
    assert!(ok(weksa(d.path(), &["scene", "dump"])).contains("entity:nibu->unnamed"));
}

#[test]
fn a_scene_names_its_addressee_as_a_participant_unless_it_is_the_speaker() {
    let d = scene_bed();
    add_line(d.path(), "nibu", "nibu", "scene-1", "Mind the hatch.");
    add_line(d.path(), "nibu", "nibu", "scene-2", "Mind the ladder.");
    ok(scene_put(d.path(), "scene-1", "entity:brokkr"));
    let dump = ok(weksa(d.path(), &["scene", "dump"]));
    assert!(dump.contains("scene:scene-1@1 rev:3"), "{dump}");
    assert!(dump.contains("[entity:nibu,entity:brokkr]"), "{dump}");
    // A speaker addressing themself is a one-participant scene, whose
    // relation to a named party is refused as the store reads it.
    let own = scene_put(d.path(), "scene-2", "entity:nibu");
    assert!(!own.status.success());
    assert!(
        out(&own).contains("a relation's named to is another participant"),
        "{}",
        out(&own)
    );
    assert_eq!(ok(weksa(d.path(), &["scene", "dump"])), dump);
}

#[test]
fn relower_refuses_malformed_arm_and_floor_labels_before_reading_the_run() {
    let d = tempfile::tempdir().unwrap();
    let relower = |arm: &str, floor: &[&str]| {
        let mut args = vec![
            "loop",
            "relower",
            "no-such-run",
            "--round",
            "1",
            "--arm",
            arm,
            "--lowerer",
            "m",
        ];
        for f in floor {
            args.extend(["--floor", f]);
        }
        weksa(d.path(), &args)
    };
    for (arm, floor) in [
        ("BAD", vec!["b"]),
        ("a", vec!["Bad"]),
        ("a", vec!["a", "b"]),
        ("a", vec!["b", "b"]),
    ] {
        let o = relower(arm, &floor);
        assert!(!o.status.success());
        assert!(
            out(&o).contains("arm and floor labels must match"),
            "{arm} {floor:?}: {}",
            out(&o)
        );
    }
    // Well-formed, distinct labels get past the label check (to the missing run).
    let o = relower("a", &["b", "c"]);
    assert!(!o.status.success());
    assert!(out(&o).contains("the root holds no run"), "{}", out(&o));
}
