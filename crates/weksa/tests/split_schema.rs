use weksa::corpus::{CorpusItem, ModelFamily, Source};
use weksa::split::fnv1a64;
use weksa::{Split, SplitScheme};

fn item(group: &str, source: Source) -> CorpusItem {
    CorpusItem {
        id: "item:t:1".into(),
        text: "t".into(),
        language: "en-US".into(),
        speaker: weksa::EntityId::parse("entity:s").unwrap(),
        names: vec![],
        group: group.into(),
        prior_turns: vec![],
        source,
        admitted_on: "2026-10-29".into(),
        supersedes: None,
        withdrawn: false,
    }
}

fn cast() -> Source {
    Source::CastAuthored {
        persona: "p".into(),
        fixture: "f".into(),
    }
}

fn model() -> Source {
    Source::ModelWritten {
        family: ModelFamily::Unrecorded,
        generator: None,
        prompt_digest: None,
    }
}

fn group_in_bucket(want: u64) -> String {
    (0..100_000)
        .map(|i| format!("g{i}"))
        .find(|g| SplitScheme::V1.bucket(g) == want)
        .unwrap_or_else(|| panic!("no group lands in bucket {want}"))
}

#[test]
fn fnv1a64_matches_the_reference_vectors() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn p1_split_deterministic() {
    let a = SplitScheme::V1.split(&item("scene-9", cast()));
    for _ in 0..3 {
        assert_eq!(SplitScheme::V1.split(&item("scene-9", cast())), a);
    }
    // The split reads the group and nothing else: speaker, text and id do not move it.
    let mut other = item("scene-9", cast());
    other.speaker = weksa::EntityId::parse("entity:someone").unwrap();
    other.text = "different".into();
    other.id = "item:other:7".into();
    assert_eq!(SplitScheme::V1.split(&other), a);
    // The bucket is the pinned hash of the prefixed group.
    assert_eq!(
        SplitScheme::V1.bucket("x"),
        fnv1a64(b"weksa-split-v1:x") % 100
    );
}

#[test]
fn split_boundaries() {
    let s = SplitScheme::V1;
    assert_eq!(s.of_bucket(0), Split::Train);
    assert_eq!(s.of_bucket(39), Split::Train);
    assert_eq!(s.of_bucket(40), Split::Validation);
    assert_eq!(s.of_bucket(79), Split::Validation);
    assert_eq!(s.of_bucket(80), Split::Test);
    assert_eq!(s.of_bucket(99), Split::Test);
    for (bucket, want) in [
        (39, Split::Train),
        (40, Split::Validation),
        (79, Split::Validation),
        (80, Split::Test),
    ] {
        assert_eq!(
            s.split(&item(&group_in_bucket(bucket), cast())),
            Some(want),
            "bucket {bucket}"
        );
    }
}

#[test]
fn p2_model_written_in_a_test_group_is_excluded() {
    let test_group = group_in_bucket(95);
    assert_eq!(
        SplitScheme::V1.split(&item(&test_group, cast())),
        Some(Split::Test)
    );
    // A model line of the test meaning is in no split; it is never moved to train.
    assert_eq!(SplitScheme::V1.split(&item(&test_group, model())), None);
    // Model-written items in the other splits keep them.
    assert_eq!(
        SplitScheme::V1.split(&item(&group_in_bucket(10), model())),
        Some(Split::Train)
    );
    assert_eq!(
        SplitScheme::V1.split(&item(&group_in_bucket(60), model())),
        Some(Split::Validation)
    );
}

#[test]
fn p3_split_ratio() {
    let mut counts = [0usize; 3];
    for i in 0..10_000 {
        match SplitScheme::V1.split(&item(&format!("scene-{i}"), cast())) {
            Some(Split::Train) => counts[0] += 1,
            Some(Split::Validation) => counts[1] += 1,
            Some(Split::Test) => counts[2] += 1,
            None => panic!("a cast item is always in a split"),
        }
    }
    let near = |got: usize, want_percent: usize| got.abs_diff(want_percent * 100) <= 200;
    assert!(
        near(counts[0], 40) && near(counts[1], 40) && near(counts[2], 20),
        "{counts:?}"
    );
}

#[test]
fn published_schemas_match() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/cultnet");
    let published = weksa::schema::published();
    let mut names: Vec<String> = published.iter().map(|(n, _)| n.clone()).collect();
    names.sort();
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".schema.json"))
        .collect();
    on_disk.sort();
    assert_eq!(
        on_disk, names,
        "schemas/cultnet holds exactly the generated files"
    );
    for (name, json) in published {
        let held = std::fs::read_to_string(dir.join(&name))
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(held, json, "{name} drifted; run `weksa schema`");
    }
}
