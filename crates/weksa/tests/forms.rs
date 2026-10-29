mod common;
use common::*;
use weksa::packet::{EncoderId, Grade, NodeId, PacketId, ProperName, Referent};
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
    referent["name"] = LINE.into();
    assert!(serde_json::from_value::<Referent>(referent.clone()).is_err());
    referent["name"] = "Nibu".into();
    referent["concept"] = "concept:01".into();
    assert!(serde_json::from_value::<Referent>(referent).is_err());
    assert!(serde_json::from_str::<Grade>("1001").is_err());
    assert_eq!(serde_json::from_str::<Grade>("1000").unwrap().get(), 1000);
    assert!(serde_json::from_str::<OptionSet>("[]").is_err());
}

#[test]
fn proper_name_bounds() {
    for ok in [
        "Nibu",
        "Brokkr Ivaldi",
        "O'Neil",
        "O\u{2019}Neil",
        "Jean-Luc",
        "A B C D",
    ] {
        assert_eq!(ProperName::parse(ok).unwrap().as_str(), ok);
    }
    // A caseless script is a name; so is a capital beyond ASCII.
    assert!(ProperName::parse("\u{5c71}\u{7530}").is_ok());
    assert!(ProperName::parse("\u{c9}owyn").is_ok());
    for bad in [
        LINE,
        "A B C D E",
        "Nibu.",
        "Nibu, Ivaldi",
        "Nibu  Ivaldi",
        " Nibu",
        "Nibu ",
        "",
        "Nibu2",
        "'Nibu",
        "-Nibu",
        "Nibu-",
        "Nibu'",
        "Nibu_Ivaldi",
    ] {
        assert!(ProperName::parse(bad).is_err(), "{bad:?}");
    }
    // 24 chars a token, 4 tokens and 64 bytes are the bounds.
    let token24 = format!("A{}", "b".repeat(23));
    assert!(ProperName::parse(&token24).is_ok());
    assert!(ProperName::parse(&format!("{token24}b")).is_err());
    let sized = |lens: [usize; 4]| {
        lens.iter()
            .map(|n| format!("A{}", "b".repeat(n - 1)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let at_limit = sized([15, 15, 15, 16]);
    assert_eq!(at_limit.len(), 64);
    assert!(ProperName::parse(&at_limit).is_ok());
    let over = sized([15, 15, 15, 17]);
    assert_eq!(over.len(), 65);
    assert!(ProperName::parse(&over).is_err());
    // The byte bound counts bytes, not characters.
    let wide = format!(
        "{0} {0} {0}",
        "\u{e9}".repeat(11).replace('\u{e9}', "\u{c9}")
    );
    assert!(wide.chars().count() < 64 && wide.len() > 64);
    assert!(ProperName::parse(&wide).is_err());
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

#[test]
fn proper_name_bound_in_chars() {
    // The token bound counts characters: 24 two-byte letters are 48 bytes and parse.
    let cyrillic = |n: usize| "\u{411}".repeat(n);
    assert_eq!(cyrillic(24).len(), 48);
    assert!(ProperName::parse(&cyrillic(24)).is_ok());
    assert!(ProperName::parse(&cyrillic(25)).is_err());
    // The 64-byte bound still counts bytes: two 48-byte tokens are 97 bytes.
    assert!(ProperName::parse(&format!("{0} {0}", cyrillic(24))).is_err());
}

#[test]
fn proper_name_is_form_only() {
    // A name is bounded text: no case rule, so lowercase, hyphenated and
    // caseless-script tokens parse.
    for ok in [
        "nibu",
        "Nibu ivaldi",
        "take-your-hand-off-that",
        "Take-your-hand-off-that",
        "\u{3131}-control-unless-you",
        "\u{5c71}\u{7530}",
    ] {
        assert_eq!(ProperName::parse(ok).unwrap().as_str(), ok);
    }
    // The bounds still hold: 5 tokens, 65 bytes, a sentence's punctuation.
    assert!(ProperName::parse("a b c d e").is_err());
    assert!(ProperName::parse(&"a".repeat(25)).is_err());
    let token = |n: usize| "a".repeat(n);
    let over = [token(15), token(15), token(15), token(17)].join(" ");
    assert_eq!(over.len(), 65);
    assert!(ProperName::parse(&over).is_err());
    assert!(ProperName::parse("take your hand, now.").is_err());
}
