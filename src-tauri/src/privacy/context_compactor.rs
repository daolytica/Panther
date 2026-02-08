use crate::privacy::PiiRedactor;

/// Compact raw context (code, errors, logs, history) into a shorter, privacy‑aware summary
/// suitable for remote LLM calls.
///
/// This is intentionally conservative: it prefers to drop detail rather than risk over‑sharing.
pub struct ContextCompactor {
    redactor: PiiRedactor,
}

impl Default for ContextCompactor {
    fn default() -> Self {
        Self {
            redactor: PiiRedactor::new(),
        }
    }
}

impl ContextCompactor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a compact description of the problem and minimal snippets.
    ///
    /// - `question`: the user question / instruction
    /// - `code_snippets`: small code fragments or file excerpts
    /// - `errors`: recent error messages or stack traces
    /// - `notes`: any extra free‑form notes
    pub fn compact(
        &self,
        question: &str,
        code_snippets: &[String],
        errors: &[String],
        notes: Option<&str>,
        custom_identifiers: &[String],
    ) -> String {
        // Redact PII from all text before building the summary.
        let redact = |text: &str| {
            self.redactor
                .redact_text(text, custom_identifiers, "context_compactor")
                .redacted_text
        };

        let safe_question = redact(question);

        let mut safe_snippets: Vec<String> = Vec::new();
        for (i, snippet) in code_snippets.iter().enumerate() {
            // Truncate overly large snippets to keep payload small.
            let s = if snippet.len() > 1200 {
                let head = &snippet[..800];
                let tail = &snippet[snippet.len().saturating_sub(300)..];
                format!("{head}\n// ... {} chars omitted ...\n{tail}", snippet.len().saturating_sub(1100))
            } else {
                snippet.clone()
            };
            safe_snippets.push(format!("Snippet {}:\n{}", i + 1, redact(&s)));
        }

        let mut safe_errors: Vec<String> = Vec::new();
        for (i, err) in errors.iter().enumerate() {
            // Keep only the last ~400 chars of very long errors.
            let e = if err.len() > 600 {
                let tail = &err[err.len().saturating_sub(400)..];
                format!("... (truncated) ...\n{}", tail)
            } else {
                err.clone()
            };
            safe_errors.push(format!("Error {}:\n{}", i + 1, redact(&e)));
        }

        let safe_notes = notes.map(|n| redact(n));

        let mut out = String::new();
        out.push_str("User Question:\n");
        out.push_str(&safe_question);
        out.push_str("\n\n");

        if !safe_snippets.is_empty() {
            out.push_str("Relevant Code / Snippets (heavily truncated):\n");
            for s in &safe_snippets {
                out.push_str("\n---\n");
                out.push_str(s);
                out.push('\n');
            }
            out.push('\n');
        }

        if !safe_errors.is_empty() {
            out.push_str("Recent Errors (truncated):\n");
            for e in &safe_errors {
                out.push_str("\n---\n");
                out.push_str(e);
                out.push('\n');
            }
            out.push('\n');
        }

        if let Some(n) = safe_notes {
            out.push_str("Additional context (summarized):\n");
            out.push_str(&n);
            out.push('\n');
        }

        out
    }
}

