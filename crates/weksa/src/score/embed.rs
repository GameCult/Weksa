//! The meaning metric's embedding port and its Ollama adapter.

use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::json;

/// Sentence embeddings. Every embedder (Ollama now, a style embedder later)
/// enters the scorer through this port.
pub trait Embedder {
    fn model(&self) -> &str;
    /// One vector per text, in order, all of one dimension.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

/// Cosine similarity. Precondition: `a` and `b` have one equal nonzero length
/// and nonzero norm; `Scorer::score` refuses embedder output that does not.
/// Violating it panics rather than reading as a score.
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    assert!(
        !a.is_empty() && a.len() == b.len(),
        "cosine needs vectors of one nonzero length, got {} and {}",
        a.len(),
        b.len()
    );
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    let denom = norm(a) * norm(b);
    assert!(denom > 0.0, "cosine of a zero vector");
    dot / denom
}

/// Ollama's `/api/embed` over plain HTTP. One server serves Huginn and the
/// loop, so batches stay small and `num_thread` is sent (without it a batch of
/// 32 timed out at 120 s under host load).
#[derive(Clone, Debug)]
pub struct OllamaEmbedder {
    pub url: String,
    pub model: String,
    /// Sent as `options.num_thread`; omitted when `None`.
    pub num_thread: Option<u32>,
    /// Texts per request, clamped to 1..=32.
    pub batch: usize,
}

impl OllamaEmbedder {
    pub const MAX_BATCH: usize = 32;

    pub fn new(url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            model: model.into(),
            num_thread: Some(8),
            batch: Self::MAX_BATCH,
        }
    }

    fn request(&self, agent: &ureq::Agent, batch: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut body = json!({ "model": self.model, "input": batch });
        if let Some(threads) = self.num_thread {
            body["options"] = json!({ "num_thread": threads });
        }
        let endpoint = format!("{}/api/embed", self.url.trim_end_matches('/'));
        let mut response = agent
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .with_context(|| format!("POST {endpoint}"))?;
        #[derive(Deserialize)]
        struct Reply {
            embeddings: Vec<Vec<f32>>,
        }
        let reply: Reply = serde_json::from_str(&response.body_mut().read_to_string()?)
            .context("ollama /api/embed reply")?;
        ensure!(
            reply.embeddings.len() == batch.len(),
            "asked for {} embeddings, got {}",
            batch.len(),
            reply.embeddings.len()
        );
        Ok(reply.embeddings)
    }
}

impl Embedder for OllamaEmbedder {
    fn model(&self) -> &str {
        &self.model
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .into();
        let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        for batch in texts.chunks(self.batch.clamp(1, Self::MAX_BATCH)) {
            out.extend(self.request(&agent, batch)?);
        }
        let dim = out[0].len();
        if dim == 0 || out.iter().any(|v| v.len() != dim) {
            bail!("embeddings disagree on dimension");
        }
        Ok(out)
    }
}
