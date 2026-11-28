//! Proofreading: a story's text goes to the fleet's AI relay with a server-held key, and what comes
//! back is checked before anyone sees it. The model names each fix by the exact words it would
//! change; the offsets are worked out here, against the text that was sent, so a model that
//! miscounts can't point a fix at the wrong words. Fixes are only ever suggestions: the editor
//! applies the ones a person picks. The text is never logged.
use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The most text one request may carry, in characters.
pub const MAX_CHARS: usize = 6000;
/// The most suggestions one answer may hold.
const MAX_FIXES: usize = 60;

pub const SYSTEM: &str = "You proofread text for parish bulletins, newsletters and flyers. Find spelling, \
grammar and punctuation mistakes and doubled or missing words. Do not change meaning, tone, names, \
dates, times, Scripture quotations or formatting, and do not rewrite for style. Reply with JSON only, \
no other text: {\"fixes\":[{\"find\":\"...\",\"replace\":\"...\",\"why\":\"...\"}]}. \"find\" is copied exactly \
from the text, as few words as make it unique. \"replace\" is the corrected words. \"why\" is under ten \
words. If nothing needs fixing, reply {\"fixes\":[]}.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    /// Offsets in characters (Unicode scalar values) of the text that was sent.
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
    pub reason: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AiError {
    /// The relay is busy, asleep or out of its daily allowance: try later.
    Unavailable,
    /// It answered with something that isn't usable.
    Bad(String),
}

/// Something that answers a system prompt and a user message.
pub trait Ai: Send + Sync + 'static {
    fn complete(&self, system: &str, user: &str) -> impl Future<Output = Result<String, AiError>> + Send;
}

/// The fleet's LLM relay (a chat-completions API).
pub struct Relay {
    url: String,
    key: String,
    model: String,
    http: reqwest::Client,
}

/// The chat-completions address for a relay `url`, which may be the relay's base
/// (`http://llm-relay:8080`, as OpenBao keeps it), its `/v1`, or the full path.
pub fn completions_url(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    if u.ends_with("/chat/completions") {
        u.to_string()
    } else if u.ends_with("/v1") {
        format!("{u}/chat/completions")
    } else {
        format!("{u}/v1/chat/completions")
    }
}

impl Relay {
    pub fn new(url: &str, key: &str, model: &str) -> Relay {
        Relay {
            url: completions_url(url),
            key: key.into(),
            model: model.into(),
            // Under the relay's own 85 s, so a slow answer ends here with a clear message.
            http: reqwest::Client::builder().timeout(Duration::from_secs(80)).build().unwrap_or_default(),
        }
    }
}

impl Ai for Relay {
    async fn complete(&self, system: &str, user: &str) -> Result<String, AiError> {
        let body = json!({
            "model": self.model,
            "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
            // Every local model reasons before it answers; a small budget comes back empty.
            "max_tokens": 4000,
            "temperature": 0,
        });
        let r = self.http.post(&self.url).bearer_auth(&self.key).json(&body).send().await.map_err(|e| {
            tracing::warn!("relay: {}", if e.is_timeout() { "timed out" } else { "unreachable" });
            AiError::Unavailable
        })?;
        let status = r.status().as_u16();
        if !(200..300).contains(&status) {
            tracing::warn!("relay: HTTP {status}");
            return Err(AiError::Unavailable);
        }
        let v: Value = r.json().await.map_err(|_| AiError::Bad("not JSON".into()))?;
        let choice = &v["choices"][0];
        let content = choice["message"]["content"].as_str().unwrap_or_default();
        // Some reasoning models leave content empty and put the answer beside their reasoning.
        let text = if content.trim().is_empty() { choice["message"]["reasoning_content"].as_str().unwrap_or_default() } else { content };
        if text.trim().is_empty() {
            tracing::warn!("relay: empty answer ({})", choice["finish_reason"].as_str().unwrap_or("?"));
            return Err(AiError::Unavailable);
        }
        Ok(text.to_string())
    }
}

#[derive(Deserialize)]
struct Fix {
    #[serde(default)]
    find: String,
    #[serde(default)]
    replace: String,
    #[serde(default)]
    why: String,
}

/// The `{"fixes": [...]}` object in a model's answer, wherever it sits in the text.
fn fixes(answer: &str) -> Option<Vec<Fix>> {
    let start = answer.find('{')?;
    let end = answer.rfind('}')?;
    #[derive(Deserialize)]
    struct Answer {
        fixes: Vec<Fix>,
    }
    serde_json::from_str::<Answer>(answer.get(start..=end)?).ok().map(|a| a.fixes)
}

/// Turns a model's answer into suggestions against `text`: each `find` is located in the text
/// (searching on from the last one, then from the start), fixes that change nothing, can't be
/// found or overlap an earlier one are dropped, and the rest come back in text order.
pub fn suggestions(text: &str, answer: &str) -> Result<Vec<Suggestion>, AiError> {
    let fixes = fixes(answer).ok_or_else(|| AiError::Bad("no fixes object".into()))?;
    let mut out: Vec<(usize, usize, Suggestion)> = Vec::new();
    let mut from = 0;
    for f in fixes.into_iter().take(MAX_FIXES) {
        if f.find.is_empty() || f.find == f.replace {
            continue;
        }
        let at = text.get(from..).and_then(|t| t.find(&f.find)).map(|i| i + from).or_else(|| text.find(&f.find));
        let Some(at) = at else { continue };
        let (b0, b1) = (at, at + f.find.len());
        if out.iter().any(|(s, e, _)| b0 < *e && *s < b1) {
            continue;
        }
        from = b1;
        let start = text[..b0].chars().count();
        let reason: String = f.why.chars().take(120).collect();
        out.push((b0, b1, Suggestion { start, end: start + f.find.chars().count(), original: f.find, replacement: f.replace, reason }));
    }
    out.sort_by_key(|(s, _, _)| *s);
    Ok(out.into_iter().map(|(_, _, s)| s).collect())
}

/// Proofreads `text`.
pub async fn proofread(ai: &impl Ai, text: &str) -> Result<Vec<Suggestion>, AiError> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let answer = ai.complete(SYSTEM, text).await?;
    suggestions(text, &answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_are_found_in_the_text_and_counted_in_characters() {
        let text = "Mass is at 9am on Sunay. Coffe after Mass. Café opens at noon. Coffe again.";
        let answer = r#"Here you go: {"fixes":[
            {"find":"Sunay","replace":"Sunday","why":"spelling"},
            {"find":"Coffe","replace":"Coffee","why":"spelling"},
            {"find":"Coffe","replace":"Coffee","why":"spelling"},
            {"find":"nowhere","replace":"x","why":"not in the text"},
            {"find":"noon","replace":"noon","why":"no change"},
            {"find":"Coffe after","replace":"Coffee after","why":"overlaps"}
        ]}"#;
        let s = suggestions(text, answer).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!((s[0].start, s[0].end, s[0].replacement.as_str()), (18, 23, "Sunday"));
        assert_eq!((s[1].start, s[1].original.as_str()), (25, "Coffe"));
        // After the é, character offsets differ from byte offsets.
        let second = text.rfind("Coffe").unwrap();
        assert_eq!(s[2].start, text[..second].chars().count());
        assert_ne!(s[2].start, second);
        let chars: Vec<char> = text.chars().collect();
        for x in &s {
            assert_eq!(chars[x.start..x.end].iter().collect::<String>(), x.original);
        }
    }

    #[test]
    fn the_relay_address_may_be_its_base() {
        for u in ["http://llm-relay:8080", "http://llm-relay:8080/", "http://llm-relay:8080/v1", "http://llm-relay:8080/v1/chat/completions"] {
            assert_eq!(completions_url(u), "http://llm-relay:8080/v1/chat/completions", "{u}");
        }
    }

    #[test]
    fn an_answer_without_fixes_is_refused_and_an_empty_one_is_fine() {
        assert!(suggestions("abc", "I think it is fine.").is_err());
        assert_eq!(suggestions("abc", r#"{"fixes":[]}"#).unwrap(), vec![]);
    }
}
