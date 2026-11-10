//! The loop's only door to a model. A model is the Claude session: it reads a
//! prompt file and writes an answer file, and a rerun picks the answer up. No
//! subprocess, network or binary lives here (golf-loop-in-claude).

use std::fmt;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::context::Prompt;

/// A model answering [`Prompt`]s and nothing else.
pub trait ModelPort {
    /// The id of the answering model, pinned by the run.
    fn model(&self) -> &str;
    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError>;
}

/// Why a prompt got no answer. Display names the kind only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortError {
    /// Nobody has answered yet.
    Unanswered,
    Io(ErrorKind),
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortError::Unanswered => f.write_str("the prompt is unanswered"),
            PortError::Io(kind) => write!(f, "io error {kind:?}"),
        }
    }
}

impl std::error::Error for PortError {}

/// Answers from files in one directory: `<digest>.answer` is the reply to the
/// prompt whose digest is `<digest>`; a prompt with no answer is written to
/// `<digest>.prompt` for whoever answers.
pub struct AnswerFiles {
    dir: PathBuf,
    model: String,
}

impl AnswerFiles {
    pub fn new(dir: PathBuf, model: String) -> Self {
        Self { dir, model }
    }
}

/// Answers read from one directory and nothing else: `<digest>.answer` is the
/// reply to the prompt whose digest is `<digest>`. A prompt with no file is
/// [`PortError::Unanswered`]; this port never creates or writes anything. The
/// one reader of answer files: [`AnswerFiles`] reads through it.
pub struct RecordedAnswers {
    dir: PathBuf,
    model: String,
}

impl RecordedAnswers {
    pub fn new(dir: PathBuf, model: String) -> Self {
        Self { dir, model }
    }
}

impl ModelPort for RecordedAnswers {
    fn model(&self) -> &str {
        &self.model
    }

    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
        match std::fs::read(self.dir.join(format!("{}.answer", prompt.digest()))) {
            Ok(bytes) => {
                String::from_utf8(bytes).map_err(|_| PortError::Io(ErrorKind::InvalidData))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Err(PortError::Unanswered),
            Err(e) => Err(PortError::Io(e.kind())),
        }
    }
}

impl ModelPort for AnswerFiles {
    fn model(&self) -> &str {
        &self.model
    }

    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
        let io = |e: std::io::Error| PortError::Io(e.kind());
        let digest = prompt.digest();
        match RecordedAnswers::new(self.dir.clone(), self.model.clone()).answer(prompt) {
            Err(PortError::Unanswered) => {}
            read => return read,
        }
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let path = self.dir.join(format!("{digest}.prompt"));
        match std::fs::read(&path) {
            Ok(held) if held == prompt.text().as_bytes() => {}
            Ok(_) => return Err(PortError::Io(ErrorKind::AlreadyExists)),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                std::fs::write(&path, prompt.text()).map_err(io)?
            }
            Err(e) => return Err(io(e)),
        }
        Err(PortError::Unanswered)
    }
}
