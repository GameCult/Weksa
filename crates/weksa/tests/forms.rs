mod common;
use common::*;
use weksa::packet::{EncoderId, Grade, NodeId, PacketId, Referent};
use weksa::{Family, InvId, OptionLabel, OptionSet};

/// Soul's probe: a whole corpus line, and the snake_case form that would
/// pass for an identifier.
const LINE: &str = "Take your hand off that control unless you want your nervous system to become my least useful circuit diagram.";
const SNAKE: &str = "take_your_hand_off_that_control_unless_you_want_your_nervous_system_to_become_my_least_useful_circuit_diagram";

#[test]
fn forms_refuse_surface_text() {
    for text in [LINE, SNAKE, "", " "] {
        assert!(NodeId::parse(text).is_err(), "NodeId {text:?}");
        assert!(PacketId::parse(text).is_err(), "PacketId {text:?}");
        assert!(EncoderId::parse(text).is_err(), "EncoderId {text:?}");
        assert!(InvId::parse(text).is_err(), "InvId {text:?}");
        assert!(OptionLabel::parse(text).is_err(), "OptionLabel {text:?}");
    }
    // Soul's extension key `nibu.<snake line>` is not an inventory id either.
    assert!(InvId::parse(&format!("extension:{SNAKE}")).is_err());
    assert!(InvId::parse(&format!("nibu.{SNAKE}")).is_err());
    // NodeId is n<k>, k canonical 1..=9999.
    for ok in ["n1", "n10", "n9999"] {
        assert_eq!(NodeId::parse(ok).unwrap().as_str(), ok);
    }
    for bad in [
        "n0", "n01", "n+1", "n-1", "n10000", "n", "1", "N1", "n1 ", "r1", "n1x",
    ] {
        assert!(NodeId::parse(bad).is_err(), "{bad:?}");
    }
    // PacketId and EncoderId are a prefix and exactly 16 lowercase hex digits.
    assert!(PacketId::parse(PACKET_ID).is_ok());
    assert!(EncoderId::parse(ENCODER_ID).is_ok());
    for bad in [
        "packet:0123456789abcde",
        "packet:0123456789abcdef0",
        "packet:0123456789ABCDEF",
        "packet:0123456789abcdeg",
        "encoder:0123456789abcdef",
        "0123456789abcdef",
        "packet:item:nibu:1:r2:abc",
    ] {
        assert!(PacketId::parse(bad).is_err(), "{bad:?}");
    }
    assert!(EncoderId::parse("packet:0123456789abcdef").is_err());
    assert!(EncoderId::parse("encoder:0123456789abcdeg").is_err());
    assert!(EncoderId::parse("encoder:0123456789ABCDEF").is_err());
    assert!(EncoderId::parse("encoder:0123456789abcde").is_err());
    assert!(EncoderId::parse("encoder:0123456789abcdef0").is_err());
}

#[test]
fn a_refusal_names_the_form_and_the_value_and_a_form_reads_back_as_written() {
    let e = NodeId::parse("n0").unwrap_err();
    assert_eq!(e.form, "NodeId");
    assert_eq!(e.got, "n0");
    assert_eq!(e.to_string(), "\"n0\" is not a NodeId");
    assert_eq!(
        Grade::new(1001).unwrap_err().to_string(),
        "\"1001\" is not a Grade"
    );
    assert_eq!(EncoderId::parse(ENCODER_ID).unwrap().as_str(), ENCODER_ID);
    assert_eq!(PacketId::parse(PACKET_ID).unwrap().as_str(), PACKET_ID);
    assert_eq!(
        EncoderId::parse(ENCODER_ID).unwrap().to_string(),
        ENCODER_ID
    );
    assert_eq!(lab("potential").as_str(), "potential");
}

#[test]
fn a_malformed_value_does_not_deserialize() {
    // A node id that is the snake line fails where a store would read it.
    let mut referent = serde_json::to_value(good().referents[0].clone()).unwrap();
    assert!(serde_json::from_value::<Referent>(referent.clone()).is_ok());
    referent["id"] = SNAKE.into();
    assert!(serde_json::from_value::<Referent>(referent.clone()).is_err());
    referent["id"] = "n1".into();
    referent["concept"] = "concept:01".into();
    assert!(serde_json::from_value::<Referent>(referent).is_err());
    assert!(serde_json::from_str::<Grade>("1001").is_err());
    assert_eq!(serde_json::from_str::<Grade>("1000").unwrap().get(), 1000);
    assert!(serde_json::from_str::<OptionSet>("[]").is_err());
}

#[test]
fn inv_id_canonical() {
    for bad in [
        "concept:01",
        "concept:+1",
        "concept:0",
        "concept:-1",
        "concept:",
        "concept:1x",
        "concept: 1",
        "nofamily:1",
        "Concept:1",
        "concept",
        "concept:99999999999",
        "concept:1:2",
    ] {
        assert!(InvId::parse(bad).is_err(), "{bad:?}");
    }
    let id = InvId::parse("concept:1").unwrap();
    assert_eq!(id.family(), Family::Concept);
    assert_eq!(id.to_string(), "concept:1");
    for f in Family::ALL {
        let id = InvId::parse(&format!("{}:12", f.prefix())).unwrap();
        assert_eq!(id.family(), f);
        assert_eq!(id.to_string(), format!("{}:12", f.prefix()));
    }
    assert_eq!(iv("extension:1").family(), Family::Extension);
}

#[test]
fn options_bounded() {
    let labels = |n: usize| (0..n).map(|i| lab(&format!("l{i}"))).collect::<Vec<_>>();
    assert!(OptionSet::new(labels(0)).is_err());
    assert!(OptionSet::new(labels(1)).is_ok());
    assert!(OptionSet::new(labels(16)).is_ok());
    assert!(OptionSet::new(labels(17)).is_err());
    assert!(OptionSet::new(vec![lab("a"), lab("a")]).is_err());
    assert!(OptionSet::new(vec![lab("a"), lab("b"), lab("a")]).is_err());
    // Soul's probe: the snake line is not a label; labels are short snake words.
    assert!(OptionLabel::parse(SNAKE).is_err());
    assert!(OptionLabel::parse(&"a".repeat(24)).is_ok());
    assert!(OptionLabel::parse(&"a".repeat(25)).is_err());
    for bad in ["A", "1a", "_a", "a-b", "a b", "a.b", "\u{e9}"] {
        assert!(OptionLabel::parse(bad).is_err(), "{bad:?}");
    }
    for ok in ["a", "a1", "a_b", "potential"] {
        assert!(OptionLabel::parse(ok).is_ok(), "{ok:?}");
    }
}

#[test]
fn grade_bound_is_the_type() {
    assert!(Grade::new(1001).is_err());
    assert_eq!(Grade::new(1000).unwrap().get(), 1000);
    assert_eq!(Grade::new(0).unwrap().get(), 0);
    // The published schema carries the same bound.
    let (_, json) = weksa::schema::published()
        .into_iter()
        .find(|(n, _)| n == "weksa.interlingua_packet.schema.json")
        .unwrap();
    let schema: serde_json::Value = serde_json::from_str(&json).unwrap();
    let grade = &schema["$defs"]["Grade"];
    assert_eq!(grade["maximum"], 1000, "{grade}");
    assert_eq!(grade["minimum"], 0, "{grade}");
    assert_eq!(schema["$defs"]["NodeId"]["pattern"], "^n[1-9][0-9]{0,3}$");
}
