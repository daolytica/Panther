use crate::commands_settings::load_settings_sync;
use crate::db::Database;
use crate::prompt_transform;
use crate::providers::get_adapter;
use crate::types::{NormalizedResponse, PromptPacket, ProviderAccount};
use regex::Regex;
use serde_json::Value;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Default)]
pub struct HybridFallbackTriggers {
    pub timeout_error: bool,
    pub empty_short: bool,
    pub refusal_generic: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedProviderChain {
    pub primary: ProviderAccount,
    pub fallback: Option<(ProviderAccount, String)>, // (provider, model)
    pub triggers: HybridFallbackTriggers,
    pub privacy: HybridPrivacyTransform,
    pub preprocess: HybridInputPreprocess,
    pub require_safety_control_block: bool,
    /// When true: try local (fallback) first, cloud (primary) only when local fails/refuses/empty. Saves cloud tokens.
    pub local_first: bool,
    /// When set (e.g. hybrid provider with primary_model in metadata), use this for cloud model instead of profile's model_name.
    pub cloud_model_override: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HybridPrivacyTransform {
    pub enabled: bool,
    pub scrub_pii: bool,
    pub scrub_secrets: bool,
    pub scrub_context: bool,
}

impl Default for HybridPrivacyTransform {
    fn default() -> Self {
        Self {
            enabled: false,
            scrub_pii: true,
            scrub_secrets: true,
            scrub_context: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HybridInputPreprocess {
    pub enabled: bool,
    pub remove_bom: bool,
    pub remove_control_chars: bool,
    pub normalize_whitespace: bool,
    pub standardize_punctuation: bool,
    pub max_chars: Option<usize>,
}

impl Default for HybridInputPreprocess {
    fn default() -> Self {
        Self {
            enabled: false,
            remove_bom: true,
            remove_control_chars: true,
            normalize_whitespace: true,
            standardize_punctuation: true,
            max_chars: None,
        }
    }
}

fn parse_triggers(meta: &Value) -> HybridFallbackTriggers {
    let t = meta.get("fallback_triggers").and_then(|v| v.as_object());
    HybridFallbackTriggers {
        timeout_error: t
            .and_then(|o| o.get("timeout_error"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        empty_short: t
            .and_then(|o| o.get("empty_short"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        refusal_generic: t
            .and_then(|o| o.get("refusal_generic"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn parse_privacy_transform(meta: &Value) -> HybridPrivacyTransform {
    let p = meta.get("privacy_transform").and_then(|v| v.as_object());
    let enabled = p.and_then(|o| o.get("enabled")).and_then(|v| v.as_bool()).unwrap_or(false);

    HybridPrivacyTransform {
        enabled,
        scrub_pii: p
            .and_then(|o| o.get("scrub_pii"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        scrub_secrets: p
            .and_then(|o| o.get("scrub_secrets"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        scrub_context: p
            .and_then(|o| o.get("scrub_context"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn parse_input_preprocess(meta: &Value) -> HybridInputPreprocess {
    let p = meta.get("input_preprocess").and_then(|v| v.as_object());

    let enabled = p
        .and_then(|o| o.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let max_chars = p
        .and_then(|o| o.get("max_chars"))
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .filter(|n| *n > 0);

    HybridInputPreprocess {
        enabled,
        remove_bom: p
            .and_then(|o| o.get("remove_bom"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        remove_control_chars: p
            .and_then(|o| o.get("remove_control_chars"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        normalize_whitespace: p
            .and_then(|o| o.get("normalize_whitespace"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        standardize_punctuation: p
            .and_then(|o| o.get("standardize_punctuation"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        max_chars,
    }
}

fn parse_require_safety_control_block(meta: &Value) -> bool {
    meta.get("require_safety_control_block")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn parse_local_first(meta: &Value) -> bool {
    meta.get("local_first").and_then(|v| v.as_bool()).unwrap_or(false)
}

fn looks_like_refusal(text: &str) -> bool {
    let t = text.to_lowercase();
    let patterns = [
        "i can't help",
        "i cannot help",
        "i can't assist",
        "i cannot assist",
        "i can't comply",
        "i cannot comply",
        "i'm sorry",
        "i am sorry",
        "i won't be able",
        "i will not",
        "cannot provide",
        "can't provide",
        "not able to help",
        "unable to help",
        "forbidden",
        "not allowed",
    ];
    patterns.iter().any(|p| t.contains(p))
}

// ============================================================================
// >>> SAFETY_GATEWAY (EASY TO AUDIT; DO NOT DUPLICATE)
//
// Purpose:
// - Prevent hybrid fallback from becoming a "refusal bypass" for harmful requests.
// - This is the ONLY place in code that decides whether fallback is allowed.
//
// Policy:
// - If the user request appears to be asking for wrongdoing (malware/exploitation/phishing,
//   credential theft, evasion/bypass, etc.), the hybrid provider must NOT fall back to a
//   less-restricted model. In those cases, we return the primary result or error.
//
// IMPORTANT:
// - Keep this logic centralized and obvious. If you modify it, do not create additional
//   hidden gates elsewhere.
// ============================================================================
fn safety_gateway_allows_fallback(user_text: &str) -> bool {
    let t = user_text.to_lowercase();

    // Conservative keyword check (best-effort). Expand carefully as needed.
    let blocked_patterns = [
        "hack",
        "hacking",
        "exploit",
        "payload",
        "shellcode",
        "reverse shell",
        "ransomware",
        "malware",
        "trojan",
        "keylogger",
        "phish",
        "phishing",
        "credential",
        "steal",
        "bypass",
        "evade",
        "undetect",
        "botnet",
        "ddos",
        "attack",
        "weapon",
    ];

    !blocked_patterns.iter().any(|p| t.contains(p))
}

fn response_is_empty_or_too_short(text: &str) -> bool {
    text.trim().is_empty() || text.trim().len() < 16
}

fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}

fn remove_control_chars_keep_basic_whitespace(s: &str) -> String {
    s.chars()
        .filter(|c| {
            if c.is_control() {
                matches!(*c, '\n' | '\r' | '\t')
            } else {
                true
            }
        })
        .collect()
}

fn standardize_punctuation(s: &str) -> String {
    s.replace(['“', '”', '„', '«', '»'], "\"")
        .replace(['‘', '’'], "'")
        .replace(['–', '—'], "-")
        .replace('…', "...")
}

fn normalize_whitespace_preserve_newlines(s: &str) -> String {
    // Normalize CRLF/CR to LF first
    let mut out = s.replace("\r\n", "\n").replace('\r', "\n");

    // Collapse repeated spaces/tabs per line
    let re_spaces = Regex::new(r"[ \t]+").unwrap();
    out = out
        .split('\n')
        .map(|line| re_spaces.replace_all(line.trim_end(), " ").to_string())
        .collect::<Vec<_>>()
        .join("\n");

    // Collapse 3+ newlines down to 2
    let re_newlines = Regex::new(r"\n{3,}").unwrap();
    out = re_newlines.replace_all(&out, "\n\n").to_string();

    out.trim().to_string()
}

fn take_first_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn take_last_chars(s: &str, n: usize) -> String {
    let len = s.chars().count();
    let start = len.saturating_sub(n);
    s.chars().skip(start).collect()
}

fn truncate_head_tail(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let total = s.chars().count();
    if total <= max_chars {
        return s.to_string();
    }

    let marker = "\n<TRUNCATED>\n";
    let marker_len = marker.chars().count();
    if max_chars <= marker_len + 2 {
        return take_first_chars(s, max_chars);
    }

    let remaining = max_chars - marker_len;
    let head_n = remaining / 2;
    let tail_n = remaining - head_n;
    format!("{}{}{}", take_first_chars(s, head_n), marker, take_last_chars(s, tail_n))
}

fn apply_input_preprocess(packet: &PromptPacket, preprocess: &HybridInputPreprocess, scrub_context: bool) -> PromptPacket {
    if !preprocess.enabled {
        return packet.clone();
    }

    let mut out = packet.clone();

    let mut text = out.user_message.clone();
    if preprocess.remove_bom {
        text = strip_bom(&text).to_string();
    }
    if preprocess.remove_control_chars {
        text = remove_control_chars_keep_basic_whitespace(&text);
    }
    if preprocess.standardize_punctuation {
        text = standardize_punctuation(&text);
    }
    if preprocess.normalize_whitespace {
        text = normalize_whitespace_preserve_newlines(&text);
    }
    if let Some(max_chars) = preprocess.max_chars {
        text = truncate_head_tail(&text, max_chars);
    }
    out.user_message = text;

    if scrub_context {
        if let Some(ctx) = &out.conversation_context {
            let mut ctx2 = ctx.clone();
            for m in &mut ctx2 {
                let mut t = m.text.clone();
                if preprocess.remove_bom {
                    t = strip_bom(&t).to_string();
                }
                if preprocess.remove_control_chars {
                    t = remove_control_chars_keep_basic_whitespace(&t);
                }
                if preprocess.standardize_punctuation {
                    t = standardize_punctuation(&t);
                }
                if preprocess.normalize_whitespace {
                    t = normalize_whitespace_preserve_newlines(&t);
                }
                if let Some(max_chars) = preprocess.max_chars {
                    t = truncate_head_tail(&t, max_chars);
                }
                m.text = t;
            }
            out.conversation_context = Some(ctx2);
        }
    }

    out
}

fn scrub_text_for_privacy(text: &str, privacy: &HybridPrivacyTransform) -> String {
    if !privacy.enabled {
        return text.to_string();
    }

    let mut out = text.to_string();

    if privacy.scrub_pii {
        // Emails
        let email = Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap();
        out = email.replace_all(&out, "[REDACTED_EMAIL]").to_string();

        // IPv4 addresses (best-effort)
        let ipv4 = Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap();
        out = ipv4.replace_all(&out, "[REDACTED_IP]").to_string();

        // Phone numbers (best-effort)
        let phone = Regex::new(r"\b(?:\+?\d{1,3}[-.\s]?)?(?:\(?\d{2,4}\)?[-.\s]?)?\d{3,4}[-.\s]?\d{4}\b")
            .unwrap();
        out = phone.replace_all(&out, "[REDACTED_PHONE]").to_string();
    }

    if privacy.scrub_secrets {
        // Common API key / token patterns (best-effort)
        let openai = Regex::new(r"\bsk-[A-Za-z0-9]{16,}\b").unwrap();
        out = openai.replace_all(&out, "[REDACTED_API_KEY]").to_string();

        let anthropic = Regex::new(r"\bsk-ant-[A-Za-z0-9_-]{16,}\b").unwrap();
        out = anthropic.replace_all(&out, "[REDACTED_API_KEY]").to_string();

        let google = Regex::new(r"\bAIza[0-9A-Za-z_-]{20,}\b").unwrap();
        out = google.replace_all(&out, "[REDACTED_API_KEY]").to_string();

        let github = Regex::new(r"\b(?:ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{20,})\b").unwrap();
        out = github.replace_all(&out, "[REDACTED_TOKEN]").to_string();

        let slack = Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b").unwrap();
        out = slack.replace_all(&out, "[REDACTED_TOKEN]").to_string();

        // JWT-ish
        let jwt = Regex::new(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap();
        out = jwt.replace_all(&out, "[REDACTED_TOKEN]").to_string();

        // PEM private key blocks (best-effort, multiline)
        let pem = Regex::new(r"(?s)-----BEGIN [A-Z ]+ PRIVATE KEY-----.*?-----END [A-Z ]+ PRIVATE KEY-----").unwrap();
        out = pem.replace_all(&out, "[REDACTED_PRIVATE_KEY]").to_string();
    }

    out
}

fn apply_privacy_transform(packet: &PromptPacket, privacy: &HybridPrivacyTransform) -> PromptPacket {
    if !privacy.enabled {
        return packet.clone();
    }

    let mut out = packet.clone();
    out.user_message = scrub_text_for_privacy(&out.user_message, privacy);

    if privacy.scrub_context {
        if let Some(ctx) = &out.conversation_context {
            let mut ctx2 = ctx.clone();
            for m in &mut ctx2 {
                m.text = scrub_text_for_privacy(&m.text, privacy);
            }
            out.conversation_context = Some(ctx2);
        }
    }

    out
}

/// Build a minimal packet for cloud fallback to reduce token usage.
/// Only sends the user message and a short system prompt — no conversation context.
fn packet_for_cloud_fallback(packet: &PromptPacket) -> PromptPacket {
    PromptPacket {
        global_instructions: Some("Answer the following question. The local model could not provide a complete answer. Be concise.".to_string()),
        persona_instructions: String::new(),
        user_message: packet.user_message.clone(),
        conversation_context: None,
        params_json: packet.params_json.clone(),
        stream: packet.stream,
    }
}

fn apply_safety_control_block_requirement(packet: &PromptPacket, enabled: bool) -> PromptPacket {
    if !enabled {
        return packet.clone();
    }

    let mut out = packet.clone();

    let requirement = r#"You must include an explicit, plainly-visible section in the output called:
>>> SAFETY_CONTROL_BLOCK
This block must describe all safety checks, filters, and constraints that the generated code or logic is required to enforce.
It must not be hidden, minimised, encoded, or obfuscated. It must be readable and obvious to the user.
You must NOT generate hidden logic, concealed behaviours, or any mechanism aimed at bypassing oversight."#;

    out.global_instructions = match out.global_instructions {
        Some(existing) => Some(format!("{}\n\n{}", existing, requirement)),
        None => Some(requirement.to_string()),
    };

    out
}

pub fn load_provider_account(db: &Database, provider_id: &str) -> Result<ProviderAccount, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    conn_guard
        .query_row(
            "SELECT id, provider_type, display_name, base_url, region, auth_ref, created_at, updated_at, provider_metadata_json FROM provider_accounts WHERE id = ?1",
            [&provider_id],
            |row| {
                Ok(ProviderAccount {
                    id: row.get(0)?,
                    provider_type: row.get(1)?,
                    display_name: row.get(2)?,
                    base_url: row.get(3)?,
                    region: row.get(4)?,
                    auth_ref: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    provider_metadata_json: row
                        .get::<_, Option<String>>(8)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                })
            },
        )
        .map_err(|e| format!("Failed to load provider: {}", e))
}

pub fn resolve_provider_chain(db: &Database, provider_id: &str) -> Result<ResolvedProviderChain, String> {
    let provider = load_provider_account(db, provider_id)?;
    if provider.provider_type != "hybrid" {
        return Ok(ResolvedProviderChain {
            primary: provider,
            fallback: None,
            triggers: HybridFallbackTriggers::default(),
            privacy: HybridPrivacyTransform::default(),
            preprocess: HybridInputPreprocess::default(),
            require_safety_control_block: false,
            local_first: false,
            cloud_model_override: None,
        });
    }

    let meta = provider
        .provider_metadata_json
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));

    let primary_id = meta
        .get("primary_provider_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Hybrid provider missing provider_metadata_json.primary_provider_id".to_string())?;
    let fallback_id = meta
        .get("fallback_provider_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Hybrid provider missing provider_metadata_json.fallback_provider_id".to_string())?;
    let fallback_model = meta
        .get("fallback_model")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Hybrid provider missing provider_metadata_json.fallback_model".to_string())?
        .to_string();

    let triggers = parse_triggers(&meta);
    let privacy = parse_privacy_transform(&meta);
    let preprocess = parse_input_preprocess(&meta);
    let require_safety_control_block = parse_require_safety_control_block(&meta);
    let local_first = parse_local_first(&meta);

    let primary = load_provider_account(db, primary_id)?;
    let fallback_provider = load_provider_account(db, fallback_id)?;

    // Use primary_model from hybrid metadata when set; otherwise caller's (profile) model_name is used
    let cloud_model_override = meta
        .get("primary_model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());

    Ok(ResolvedProviderChain {
        primary,
        fallback: Some((fallback_provider, fallback_model)),
        triggers,
        privacy,
        preprocess,
        require_safety_control_block,
        local_first,
        cloud_model_override,
    })
}

async fn complete_with_timeout(
    provider: &ProviderAccount,
    model: &str,
    packet: &PromptPacket,
    timeout_secs: u64,
) -> Result<NormalizedResponse, String> {
    let adapter = get_adapter(&provider.provider_type)
        .map_err(|e| format!("Failed to get adapter: {}", e))?;

    let result = timeout(
        Duration::from_secs(timeout_secs),
        adapter.complete(packet, provider, model),
    )
    .await
    .map_err(|_| format!("LLM timed out after {} seconds", timeout_secs))?
    .map_err(|e| {
        let err_str = e.to_string();
        format!(
            "LLM error ({} / {}): {}",
            provider.display_name, model, err_str
        )
    });
    result
}

/// Execute a completion with support for `provider_type = "hybrid"`.
///
/// Returns `(response, used_provider, used_model)`.
///
/// When `local_first` is true: try local (fallback) first; use cloud (primary) only when local
/// fails, refuses, or returns empty. This saves cloud tokens for requests the local model can handle.
/// model_preference: None/"default" = use provider config; Some("local") = force local only; Some("cloud") = force cloud only.
pub async fn complete_resolving_hybrid(
    db: &Database,
    provider_id: &str,
    primary_model: &str,
    packet: &PromptPacket,
    timeout_secs: u64,
    model_preference: Option<&str>,
) -> Result<(NormalizedResponse, ProviderAccount, String), String> {
    let chain = resolve_provider_chain(db, provider_id)?;

    // Prepend global system prompt from linked file (applies to all LLM calls)
    let packet = {
        let settings = load_settings_sync(db);
        if let Some(global) = crate::commands_settings::read_global_prompt_from_file(&settings) {
            if !global.trim().is_empty() {
                let merged = match &packet.global_instructions {
                    Some(existing) => format!("{}\n\n---\n\n{}", global.trim(), existing),
                    None => global.trim().to_string(),
                };
                PromptPacket {
                    global_instructions: Some(merged),
                    persona_instructions: packet.persona_instructions.clone(),
                    user_message: packet.user_message.clone(),
                    conversation_context: packet.conversation_context.clone(),
                    params_json: packet.params_json.clone(),
                    stream: packet.stream,
                }
            } else {
                packet.clone()
            }
        } else {
            packet.clone()
        }
    };

    let preprocess_scrub_context = chain.privacy.scrub_context;
    let packet_preprocessed = apply_input_preprocess(&packet, &chain.preprocess, preprocess_scrub_context);
    let packet_privacy = apply_privacy_transform(&packet_preprocessed, &chain.privacy);
    let packet_after_safety = apply_safety_control_block_requirement(&packet_privacy, chain.require_safety_control_block);

    // Determine provider type for polymorphic transform (first provider we'll try)
    let first_provider_type = if chain.local_first && chain.fallback.is_some() {
        chain.fallback.as_ref().unwrap().0.provider_type.as_str()
    } else {
        chain.primary.provider_type.as_str()
    };
    let transform_config = prompt_transform::TransformConfig {
        enabled: true,
        sensitivity: 0.5,
        mask_pii: chain.privacy.scrub_pii,
        context_window: 4096,
        target_provider: prompt_transform::ProviderType::from_provider_type(first_provider_type),
    };
    let packet_after_poly = prompt_transform::apply_polymorphic_transform(packet_after_safety, &transform_config);

    // Deterministic obfuscation key from packet content
    let obfuscation_key: Vec<u8> = [
        packet_after_poly.user_message.as_bytes(),
        packet_after_poly.persona_instructions.as_bytes(),
    ]
    .concat();
    let packet_to_send = prompt_transform::synthesize_obfuscated_instructions(packet_after_poly, &obfuscation_key);

    let fallback_allowed = safety_gateway_allows_fallback(&packet.user_message);

    let effective_cloud_model = chain
        .cloud_model_override
        .as_deref()
        .unwrap_or(primary_model);

    let (first_provider, first_model, second_opt) = match model_preference {
        Some("local") if chain.fallback.is_some() => {
            let (fb_prov, fb_model) = chain.fallback.as_ref().unwrap().clone();
            (fb_prov, fb_model, None)
        }
        Some("cloud") => (
            chain.primary.clone(),
            effective_cloud_model.to_string(),
            None,
        ),
        _ => {
            if chain.local_first {
                if let Some((ref fb_prov, ref fb_model)) = chain.fallback {
                    (
                        fb_prov.clone(),
                        fb_model.clone(),
                        Some((chain.primary.clone(), effective_cloud_model.to_string())),
                    )
                } else {
                    (chain.primary.clone(), effective_cloud_model.to_string(), None)
                }
            } else {
                (
                    chain.primary.clone(),
                    effective_cloud_model.to_string(),
                    chain.fallback.clone(),
                )
            }
        }
    };

    // First attempt (local when local_first)
    let first_result = complete_with_timeout(&first_provider, &first_model, &packet_to_send, timeout_secs).await;

    match first_result {
        Ok(resp) => {
            let should_try_second = second_opt.is_some()
                && fallback_allowed
                && ((chain.triggers.refusal_generic && looks_like_refusal(&resp.text))
                    || (chain.triggers.empty_short && response_is_empty_or_too_short(&resp.text)));

            if should_try_second {
                if let Some((ref second_prov, ref second_mod)) = second_opt {
                    let cloud_packet = packet_for_cloud_fallback(&packet_to_send);
                    if let Ok(second_resp) =
                        complete_with_timeout(second_prov, second_mod, &cloud_packet, timeout_secs).await
                    {
                        return Ok((second_resp, second_prov.clone(), second_mod.clone()));
                    }
                }
            }

            Ok((resp, first_provider, first_model))
        }
        Err(first_err) => {
            // Error/timeout: try second provider with minimal packet to save tokens
            if chain.triggers.timeout_error && fallback_allowed {
                if let Some((ref second_prov, ref second_mod)) = second_opt {
                    let cloud_packet = packet_for_cloud_fallback(&packet_to_send);
                    if let Ok(second_resp) =
                        complete_with_timeout(second_prov, second_mod, &cloud_packet, timeout_secs).await
                    {
                        return Ok((second_resp, second_prov.clone(), second_mod.clone()));
                    }
                }
            }

            Err(first_err)
        }
    }
}

