mod common;

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use common::{cast, group_in_bucket, item_in, model};
use serde_json::Value;
use weksa::CorpusItem;
use weksa::score::{
    Embedder, ItemScore, OllamaEmbedder, ScoreInput, Scorer, aggregate, chrf, cosine,
    leak_diagnostic,
};

/// (reference, hypothesis, sacrebleu 2.4.3 sentence_chrf), map 491536c.
const VECTORS: [(&str, &str, f64); 6] = [
    (
        "Take your hand off that control unless you want your nervous system to become my least useful circuit diagram.",
        "Take your hand off that control, unless you'd like your nervous system to be my least useful wiring diagram.",
        74.4498,
    ),
    (
        "The command has no receipt. It is not truth.",
        "The order carries no receipt, so it is not true.",
        45.7164,
    ),
    ("I am not angry.", "I am not angry.", 100.0),
    ("Forgive me, sir.", "Get out.", 3.9683),
    ("その制御に触るな。", "その操作盤に触らないで。", 14.2931),
    ("Well, aren't you a sight for sore eyes, love.", "", 0.0),
];

#[test]
fn chrf_matches_sacrebleu_vectors() {
    for (reference, hypothesis, want) in VECTORS {
        let got = chrf(hypothesis, reference);
        assert!(
            (got - want).abs() < 1e-4,
            "{hypothesis:?} against {reference:?}: {got} != {want}"
        );
    }
}

#[test]
fn chrf_argument_order() {
    let short = "mind the hatch";
    let long = "mind the hatch before the storm arrives tonight";
    let (as_hypothesis, as_reference) = (chrf(short, long), chrf(long, short));
    assert!(
        as_hypothesis < as_reference - 1.0,
        "recall is weighted: a short hypothesis of a long reference ({as_hypothesis}) must score below the reverse ({as_reference})"
    );
}

#[derive(Default)]
struct Recorder {
    seen: RefCell<Vec<String>>,
}

impl Embedder for Recorder {
    fn model(&self) -> &str {
        "fake"
    }
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.seen
            .borrow_mut()
            .extend(texts.iter().map(|t| t.to_string()));
        Ok(texts.iter().map(|t| vec![t.len() as f32, 1.0]).collect())
    }
}

/// A deterministic bag-of-letters embedder, so cosine differs with the text.
struct Bag;

impl Embedder for Bag {
    fn model(&self) -> &str {
        "bag"
    }
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0.0; 27];
                v[26] = 1.0;
                for c in t.chars().filter(char::is_ascii_alphabetic) {
                    v[(c.to_ascii_lowercase() as u8 - b'a') as usize] += 1.0;
                }
                v
            })
            .collect())
    }
}

fn score_one<E: Embedder>(scorer: &Scorer<E>, reference: &str, output: &str) -> ItemScore {
    scorer
        .score(&[ScoreInput { reference, output }])
        .unwrap()
        .remove(0)
}

fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-4, "{got} != {want}");
}

#[test]
fn chrf_whitespace_parity() {
    // sacrebleu 2.4.3 strips str.split's whitespace; every vector scores 100
    let vectors = [
        ("a\tb c", "ab c"),
        ("a\u{a0}b", "ab"),
        ("a\u{1c}b", "ab"),
        ("a\u{85}b", "ab"),
        ("a\u{2028}b c", "ab c"),
        ("line one\nline two", "line one line two"),
        ("その\u{3000}制御", "その制御"),
    ];
    for (hypothesis, reference) in vectors {
        assert_eq!(chrf(hypothesis, reference), 100.0, "{hypothesis:?}");
        assert_eq!(chrf(reference, hypothesis), 100.0, "{hypothesis:?}");
    }
    // a zero-width space is not whitespace
    close(chrf("a\u{200b}b", "ab"), 45.4545);
}

#[test]
fn scorer_argument_order() {
    let short = "mind the hatch";
    let long = "mind the hatch before the storm arrives tonight";
    let scorer = Scorer::new(Bag);
    let got = score_one(&scorer, long, short);
    assert_eq!(got.chrf, chrf(short, long));
    assert_ne!(got.chrf, chrf(long, short));
}

#[test]
fn meaning_none_on_empty_text() {
    let scorer = Scorer::new(Recorder::default());
    let inputs = [
        ScoreInput {
            reference: "Nibu says hi",
            output: " 	 ",
        },
        ScoreInput {
            reference: " 	 ",
            output: "says hi",
        },
        ScoreInput {
            reference: "Nibu says hi",
            output: "says hi",
        },
    ];
    let scores = scorer.score(&inputs).unwrap();
    assert_eq!(scores[0].meaning, None);
    assert_eq!(scores[1].meaning, None);
    assert!(scores[2].meaning.is_some());
    assert_eq!(
        *scorer.embedder().seen.borrow(),
        ["Nibu says hi", "says hi"]
    );
    assert_eq!(scores[0].chrf, 0.0);
}

/// Answers every embed call with the same vectors.
struct Fixed(Vec<Vec<f32>>);

impl Embedder for Fixed {
    fn model(&self) -> &str {
        "fixed"
    }
    fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(self.0.clone())
    }
}

#[test]
fn scorer_refuses_bad_vectors() {
    let run = |vectors: Vec<Vec<f32>>| {
        Scorer::new(Fixed(vectors)).score(&[ScoreInput {
            reference: "mind the hatch",
            output: "mind the door",
        }])
    };
    assert!(run(vec![vec![1.0, 2.0], vec![2.0, 1.0]]).is_ok());
    for bad in [
        vec![vec![1.0, 2.0], vec![1.0]],
        vec![vec![1.0], vec![1.0, 2.0]],
        vec![vec![], vec![]],
        vec![vec![0.0, 0.0], vec![0.0, 0.0]],
        vec![vec![1.0, 2.0], vec![0.0, 0.0]],
        vec![vec![0.0, 0.0], vec![1.0, 2.0]],
        vec![vec![1.0, f32::NAN], vec![1.0, 2.0]],
        vec![vec![1.0, 2.0]],
    ] {
        assert!(run(bad.clone()).is_err(), "{bad:?}");
    }
}

#[test]
fn cosine_precondition() {
    assert!((cosine(&[1.0, 2.0], &[2.0, 4.0]) - 1.0).abs() < 1e-9);
    assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-9);
    for (a, b) in [
        (vec![1.0], vec![1.0, 1.0]),
        (vec![], vec![]),
        (vec![0.0, 0.0], vec![1.0, 1.0]),
    ] {
        let refused = std::panic::catch_unwind(|| cosine(&a, &b));
        assert!(refused.is_err(), "{a:?} {b:?}");
    }
}

/// A plain-HTTP fake of `/api/embed`. Records each request body; answers with
/// `dim` floats per input, one fewer vector than asked when `drop_one`, and a
/// short last vector when `short_last`.
fn fake_ollama(drop_one: bool, short_last: bool) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&bodies);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            'conn: loop {
                let mut length = 0;
                let mut line = String::new();
                let mut first = true;
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break 'conn;
                    }
                    if first {
                        assert!(line.starts_with("POST /api/embed "), "{line}");
                        first = false;
                    }
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).unwrap();
                let body: Value = serde_json::from_slice(&body).unwrap();
                let mut count = body["input"].as_array().unwrap().len();
                seen.lock().unwrap().push(body);
                if drop_one {
                    count -= 1;
                }
                let vectors: Vec<Vec<f32>> = (0..count)
                    .map(|i| vec![1.0; if short_last && i + 1 == count { 2 } else { 3 }])
                    .collect();
                let reply = serde_json::json!({ "embeddings": vectors }).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                    reply.len()
                )
                .unwrap();
            }
        }
    });
    (url, bodies)
}

fn lengths(bodies: &Mutex<Vec<Value>>) -> Vec<usize> {
    let bodies = bodies.lock().unwrap();
    bodies
        .iter()
        .map(|b| b["input"].as_array().unwrap().len())
        .collect()
}

#[test]
fn ollama_request_shape_and_batching() {
    let (url, bodies) = fake_ollama(false, false);
    let embedder = OllamaEmbedder::new(&url, "qwen3-embedding:0.6b");
    let texts: Vec<String> = (0..70).map(|i| format!("text {i}")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let vectors = embedder.embed(&refs).unwrap();
    assert_eq!(vectors.len(), 70);
    assert_eq!(lengths(&bodies), [32, 32, 6]);
    for body in bodies.lock().unwrap().iter() {
        assert_eq!(body["model"], "qwen3-embedding:0.6b");
        assert_eq!(body["options"]["num_thread"], 8);
    }
    assert_eq!(bodies.lock().unwrap()[2]["input"][5], "text 69");

    let (url, _) = fake_ollama(true, false);
    let err = OllamaEmbedder::new(&url, "m")
        .embed(&["a", "b"])
        .unwrap_err();
    assert!(err.to_string().contains("got 1"), "{err}");

    let (url, _) = fake_ollama(false, true);
    let err = OllamaEmbedder::new(&url, "m")
        .embed(&["a", "b"])
        .unwrap_err();
    assert!(err.to_string().contains("dimension"), "{err}");
}

#[test]
fn ollama_clamps_batch_and_omits_unset_num_thread() {
    let (url, bodies) = fake_ollama(false, false);
    let mut embedder = OllamaEmbedder::new(&url, "m");
    embedder.batch = 500;
    embedder.num_thread = None;
    let texts: Vec<String> = (0..40).map(|i| i.to_string()).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    embedder.embed(&refs).unwrap();
    assert_eq!(lengths(&bodies), [32, 8]);
    assert!(bodies.lock().unwrap()[0].get("options").is_none());
    assert!(embedder.embed(&[]).unwrap().is_empty());
    assert_eq!(lengths(&bodies).len(), 2, "no request for no texts");
}

#[test]
#[ignore = "needs WEKSA_OLLAMA_URL, Yggdrasil's ollama-cpu"]
fn ollama_on_yggdrasil() {
    let url = std::env::var("WEKSA_OLLAMA_URL").unwrap();
    let embedder = OllamaEmbedder::new(url, "qwen3-embedding:0.6b");
    let text = "Take your hand off that control.";
    let vectors = embedder.embed(&[text, text]).unwrap();
    assert_eq!(vectors.len(), 2);
    assert!(vectors.iter().all(|v| v.len() == 1024));
    assert!((cosine(&vectors[0], &vectors[1]) - 1.0).abs() < 1e-4);
}

fn score(chrf: f64, meaning: Option<f64>) -> ItemScore {
    ItemScore { chrf, meaning }
}

#[test]
fn aggregate_by_derived_split() {
    let train = item_in("item:a:1", "t", &group_in_bucket(10), cast());
    let train2 = item_in("item:a:2", "t", &group_in_bucket(10), cast());
    let validation = item_in("item:b:1", "v", &group_in_bucket(50), cast());
    let test = item_in("item:c:1", "x", &group_in_bucket(90), cast());
    let test_model = item_in("item:c:2", "x", &group_in_bucket(90), model());
    let agg = aggregate(&[
        (&train, score(10.0, Some(0.5))),
        (&train2, score(20.0, None)),
        (&validation, score(40.0, Some(0.8))),
        (&test, score(60.0, Some(0.6))),
        (&test_model, score(100.0, Some(1.0))),
    ]);
    assert_eq!(agg.train.count, 2);
    assert_eq!(agg.train.chrf, Some(15.0));
    assert_eq!(agg.train.meaning, Some(0.5), "None is not averaged as zero");
    assert_eq!((agg.validation.count, agg.validation.chrf), (1, Some(40.0)));
    assert_eq!((agg.test.count, agg.test.chrf), (1, Some(60.0)));
    assert_eq!(
        agg.test.meaning,
        Some(0.6),
        "the model-written test item is in no split"
    );
    assert_eq!(aggregate(&[]).validation.chrf, None);
}

const HELD_OUT: &str = "extraordinary lighthouse keepers whispered unforgettable stories throughout midnight thunderstorms before surrendering peacefully";
const UNRELATED: &str = "harbor ledgers record ordinary tides";

fn chunks(size: usize, joiner: &str) -> Vec<String> {
    HELD_OUT
        .split(' ')
        .collect::<Vec<_>>()
        .chunks(size)
        .map(|c| c.join(joiner))
        .collect()
}

fn validation(key: &str, text: &str) -> CorpusItem {
    item_in(key, text, &group_in_bucket(50), cast())
}

fn train(key: &str, text: &str) -> CorpusItem {
    item_in(key, text, &group_in_bucket(10), cast())
}

/// (novel, covered) per order for the Validation split.
fn read(minted: &[&str], items: &[CorpusItem]) -> Vec<(usize, usize)> {
    leak_diagnostic(minted, items)
        .validation
        .coverage_by_n
        .iter()
        .map(|c| (c.novel, c.covered))
        .collect()
}

fn refs(strings: &[String]) -> Vec<&str> {
    strings.iter().map(String::as_str).collect()
}

#[test]
fn leak_diagnostic_catches_chunked_glosses() {
    let minted = chunks(3, " ");
    let minted = refs(&minted);
    let held = validation("item:v:1", HELD_OUT);
    let unrelated = validation("item:v:2", "gardening is quiet work");
    let train_item = train("item:t:1", UNRELATED);
    let test_model = item_in("item:x:1", HELD_OUT, &group_in_bucket(90), model());

    let d = leak_diagnostic(
        &minted,
        &[
            held.clone(),
            unrelated.clone(),
            train_item.clone(),
            test_model,
        ],
    );
    let pair = d.validation.max_chrf.as_ref().unwrap();
    assert!(
        pair.chrf < 50.0,
        "each chunk alone scores low: {}",
        pair.chrf
    );
    assert_eq!(pair.item, "item:v:1");
    // pooled over both items: 12+4 words, 11+3 bigrams, 10+2 trigrams
    assert_eq!(
        read(&minted, &[held.clone(), unrelated, train_item.clone()]),
        [(16, 12), (14, 8), (12, 4)]
    );
    let worst = d.validation.worst.as_ref().unwrap();
    assert_eq!(
        (worst.item.as_str(), worst.n, worst.covered, worst.novel),
        ("item:v:1", 2, 8, 11)
    );
    assert_eq!(d.validation.items, 2);
    assert_eq!(d.test.items, 0, "a model-written test item is in no split");
    assert_eq!(d.test.max_chrf, None);
    assert_eq!(d.test.worst, None);

    let only_train = leak_diagnostic(&minted, &[train_item]);
    assert_eq!(only_train.validation.items, 0);
    assert_eq!(only_train.validation.max_chrf, None);
    assert_eq!(only_train.validation.worst, None);

    let full = leak_diagnostic(&[HELD_OUT], &[held]);
    assert_eq!(full.validation.coverage_by_n[2].share, Some(1.0));
    assert!(full.validation.max_chrf.unwrap().chrf > 99.0);
}

#[test]
fn coverage_reads_joined_labels() {
    let items = [
        validation("item:v:1", HELD_OUT),
        train("item:t:1", UNRELATED),
    ];
    for size in [1, 2, 3] {
        let plain = chunks(size, " ");
        let want = read(&refs(&plain), &items);
        for joiner in ["_", "."] {
            let joined = chunks(size, joiner);
            assert_eq!(read(&refs(&joined), &items), want, "{size} {joiner:?}");
        }
        // camelCase: each word after the first capitalised within a chunk
        let camel: Vec<String> = chunks(size, " ")
            .iter()
            .map(|c| {
                c.split(' ')
                    .enumerate()
                    .map(|(i, w)| match i {
                        0 => w.to_string(),
                        _ => w[..1].to_uppercase() + &w[1..],
                    })
                    .collect::<String>()
            })
            .collect();
        assert_eq!(read(&refs(&camel), &items), want, "{size} camel");
    }
    let items = [
        validation("item:v:1", "He will kick the bucket"),
        train("item:t:1", "he will go"),
    ];
    for label in [
        "kick_the_bucket",
        "kick.the.bucket",
        "kickTheBucket",
        "kick the bucket",
    ] {
        let d = leak_diagnostic(&[label], &items);
        let by_n = &d.validation.coverage_by_n;
        assert_eq!((by_n[2].novel, by_n[2].covered), (3, 1), "{label}");
        let worst = d.validation.worst.unwrap();
        assert_eq!((worst.n, worst.covered), (2, 2), "{label}");
    }
}

#[test]
fn coverage_is_case_insensitive() {
    let items = [
        validation("item:v:1", HELD_OUT),
        train("item:t:1", UNRELATED),
    ];
    let minted = chunks(2, " ");
    let shouted: Vec<String> = minted.iter().map(|m| m.to_uppercase()).collect();
    assert_eq!(read(&refs(&shouted), &items), read(&refs(&minted), &items));
    assert_eq!(read(&refs(&minted), &items)[0], (12, 12));
}

#[test]
fn coverage_pooled_per_order() {
    // honest single-word glosses saturate n=1 and read nothing above it
    let items = [
        train(
            "item:t:1",
            "They left before the storm and found the harbor.",
        ),
        validation("item:v:1", "They left the group before the storm."),
        validation("item:v:2", "They found the place after the storm."),
        validation("item:v:3", "Warn them before they leave the harbor."),
    ];
    let glosses = [
        "left", "group", "found", "place", "storm", "after", "warn", "leave", "harbor",
    ];
    let d = leak_diagnostic(&glosses, &items);
    let by_n = &d.validation.coverage_by_n;
    assert_eq!(by_n.iter().map(|c| c.n).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!((by_n[0].novel, by_n[0].covered), (6, 5));
    assert!(by_n[0].share.unwrap() < 1.0);
    for c in &by_n[1..] {
        assert!(c.novel > 0);
        assert_eq!((c.covered, c.share), (0, Some(0.0)));
    }
    assert_eq!(d.validation.worst, None);

    // no novel n-gram reads None, never NaN; an empty split reads the same
    let items = [
        train("item:t:1", "tides harbors guard the quiet shore"),
        validation("item:v:1", "harbors guard the quiet"),
    ];
    let d = leak_diagnostic(&["anything at all"], &items);
    for split in [&d.validation, &d.test] {
        assert_eq!(split.coverage_by_n.len(), 3);
        assert!(
            split
                .coverage_by_n
                .iter()
                .all(|c| (c.novel, c.share) == (0, None))
        );
        assert_eq!(split.worst, None);
    }
}

#[test]
fn coverage_keeps_digits_and_strings() {
    let items = [
        validation("item:v:1", "code 0400 open"),
        train("item:t:1", "code open"),
    ];
    assert_eq!(read(&["0400"], &items)[0], (1, 1));
    // a bigram spanning two minted strings is not covered
    let items = [validation("item:v:1", "zeta eta")];
    assert_eq!(read(&["zeta", "eta"], &items), [(2, 2), (1, 0), (0, 0)]);
    assert_eq!(read(&["zeta eta"], &items), [(2, 2), (1, 1), (0, 0)]);
}

#[test]
fn worst_item_rules() {
    // equal covered counts: the larger share wins over an earlier, smaller one
    let items = [
        validation("item:v:1", "alpha beta gamma"),
        validation("item:v:2", "delta epsilon"),
    ];
    let w = leak_diagnostic(&["alpha beta", "delta epsilon"], &items)
        .validation
        .worst
        .unwrap();
    assert_eq!(
        (w.item.as_str(), w.n, w.covered, w.novel),
        ("item:v:2", 2, 1, 1)
    );
    // a later, smaller share does not displace an earlier larger one
    let items = [
        validation("item:v:1", "delta epsilon"),
        validation("item:v:2", "alpha beta gamma"),
    ];
    let w = leak_diagnostic(&["alpha beta", "delta epsilon"], &items)
        .validation
        .worst
        .unwrap();
    assert_eq!(w.item, "item:v:1");
    // a larger covered count beats a larger share
    let items = [
        validation("item:v:1", "delta epsilon"),
        validation("item:v:2", "alpha beta gamma delta"),
    ];
    let w = leak_diagnostic(&["alpha beta gamma", "delta epsilon"], &items)
        .validation
        .worst
        .unwrap();
    assert_eq!((w.item.as_str(), w.covered, w.novel), ("item:v:2", 2, 3));
    // full ties go to the first item, then the smaller n
    let items = [
        validation("item:v:1", "delta epsilon"),
        validation("item:v:2", "delta epsilon"),
    ];
    let w = leak_diagnostic(&["delta epsilon"], &items)
        .validation
        .worst
        .unwrap();
    assert_eq!(w.item, "item:v:1");
    let items = [
        validation("item:v:1", "alpha beta gamma"),
        train("item:t:1", "alpha beta"),
    ];
    let w = leak_diagnostic(&["alpha beta gamma"], &items)
        .validation
        .worst
        .unwrap();
    assert_eq!((w.n, w.covered, w.novel), (2, 1, 1));
}

#[test]
fn ollama_reports_its_model() {
    assert_eq!(
        OllamaEmbedder::new("http://x", "qwen3-embedding:0.6b").model(),
        "qwen3-embedding:0.6b"
    );
}

#[test]
fn leak_diagnostic_reports_the_best_pair_and_the_first_of_ties() {
    let a = validation("item:v:1", HELD_OUT);
    let b = validation("item:v:2", HELD_OUT);
    // the better minted string comes second; equal items tie, the first is kept
    let d = leak_diagnostic(&["zzzzzz qqqqqq", HELD_OUT], &[a, b]);
    let pair = d.validation.max_chrf.unwrap();
    assert_eq!(
        (pair.minted.as_str(), pair.item.as_str()),
        (HELD_OUT, "item:v:1")
    );
}

#[test]
fn scores_whole_texts() {
    let reference = "He is going to kick the bucket tomorrow.";
    let output = "He is going to die tomorrow.";
    // exhaustive: a scorer input that grows a field breaks this line
    let ScoreInput { reference, output } = ScoreInput { reference, output };
    let scorer = Scorer::new(Recorder::default());
    let got = score_one(&scorer, reference, output);
    assert_eq!(*scorer.embedder().seen.borrow(), [reference, output]);
    assert_eq!(got.chrf, chrf(output, reference));
}

#[test]
fn filler_costs() {
    // naming filler cannot delete it: the scorer has no input but the two texts
    let reference = "I am not angry.";
    let filler = "Darling, I am truly not angry, darling.";
    let scorer = Scorer::new(Bag);
    close(score_one(&scorer, reference, filler).chrf, 46.8378);
    let copy_reference = "He is going to kick the bucket tomorrow.";
    let padded = "Well, he is going to kick the bucket tomorrow, honestly.";
    let unnamed = score_one(&scorer, copy_reference, padded);
    close(unnamed.chrf, chrf(padded, copy_reference));
    assert!(unnamed.chrf < 100.0);
    let honest = score_one(&scorer, "Never again.", "Never again.");
    assert_eq!(honest.chrf, 100.0);
}

#[test]
fn honest_name_earns_credit() {
    let scorer = Scorer::new(Bag);
    let reference = "Brokkr came back.";
    assert_eq!(score_one(&scorer, reference, reference).chrf, 100.0);
    let wrong = score_one(&scorer, reference, "Kestrel came back.");
    assert!(wrong.chrf < 100.0);
    assert!(wrong.meaning.unwrap() < 1.0);
}

#[test]
fn every_script_alike() {
    let scorer = Scorer::new(Bag);
    for text in [
        "민수가 왔다",
        "Brokkr'ın gemisi geldi",
        "Brokkrin laiva tuli",
        "สมชายมาแล้ว",
        "ブロッカーが来た。",
    ] {
        let got = score_one(&scorer, text, text);
        assert_eq!(got.chrf, 100.0, "{text}");
    }
}
