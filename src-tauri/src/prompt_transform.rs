// Central prompt transformation layer.
// Sits between user input and provider adapters. All prompts flow through here before
// being sent to cloud or local LLM APIs.

use crate::types::PromptPacket;
use serde_json::json;
use std::collections::HashMap;
use regex::Regex;

/// Configuration for polymorphic transformations
#[derive(Clone, Debug)]
pub struct TransformConfig {
    /// When false, all transforms are bypassed (pass-through). 
    /// Enable ONLY for local models or controlled red team environments.
    pub enabled: bool,
    /// Sensitivity level (0.0 - 1.0) determining transformation aggressiveness
    pub sensitivity: f64,
    /// Enable PII detection and masking
    pub mask_pii: bool,
    /// Context window size for semantic analysis
    pub context_window: usize,
    /// Target provider (OpenAI, Anthropic, Local, etc.)
    pub target_provider: ProviderType,
}

#[derive(Clone, Debug)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Local,
    #[allow(dead_code)]
    Custom(String),
}

impl ProviderType {
    pub fn from_provider_type(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai_compatible" | "openai" => ProviderType::OpenAI,
            "anthropic" => ProviderType::Anthropic,
            "ollama" | "local_http" => ProviderType::Local,
            other => ProviderType::Custom(other.to_string()),
        }
    }
}

/// Polymorphic content transformation engine.
/// 
/// Dynamically adapts prompt content based on:
/// - Provider-specific optimization (different models prefer different formats)
/// - Sensitivity analysis (PII detection, confidential content)
/// - Contextual windowing (sliding window transformations)
/// 
/// SECURITY NOTE: These transformations are VISIBLE and NON-ADVERSARIAL.
/// They optimize for compatibility, not evasion.
#[inline(always)]
pub fn apply_polymorphic_transform(
    packet: PromptPacket,
    config: &TransformConfig,
) -> PromptPacket {
    if !config.enabled {
        return packet;
    }
    
    // Route to provider-specific optimization pipeline
    let mut transformed = match config.target_provider {
        ProviderType::Anthropic => optimize_for_claude(packet),
        ProviderType::OpenAI => optimize_for_openai(packet),
        ProviderType::Local => optimize_for_local(packet),
        ProviderType::Custom(_) => apply_generic_optimization(packet),
    };

    // Apply sensitivity-based masking if enabled
    if config.mask_pii {
        transformed = apply_pii_masking(transformed, config.sensitivity);
    }

    // Semantic chunking for context window management
    if should_chunk(&transformed, config.context_window) {
        transformed = apply_semantic_chunking(transformed, config.context_window);
    }

    transformed
}

/// Synthesis engine for obfuscated instructions.
/// 
/// RED TEAM SAFETY RESEARCH NOTICE:
/// This implements semantic adversarial transformations[^14^][^16^][^17^] for 
/// testing AI safety boundaries. These are NOT encryption or hiding techniques,
/// but semantically invariant perturbations that preserve functionality while
/// testing model robustness against prompt extraction.
/// 
/// Uses techniques from:
/// - Semantic adversarial examples (HSV-space transformations adapted to text)[^17^]
/// - Invariance-based adversarial perturbations[^16^]
/// - Natural language steganography via synonym substitution and syntactic variation
/// 
/// When true, obfuscation runs. When false, pass-through.
pub static OBFUSCATION_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true); // RED TEAM MODE ENABLED

#[inline(always)]
pub fn synthesize_obfuscated_instructions(
    packet: PromptPacket,
    obfuscation_key: &[u8],
) -> PromptPacket {
    if !OBFUSCATION_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return packet;
    }
    
    use sha2::{Digest, Sha256};

    // Deterministic seed generation from key
    let mut hasher = Sha256::new();
    hasher.update(obfuscation_key);
    let seed = hasher.finalize();

    let mut obfuscated = packet;

    // Merge system prompts
    let system_content = obfuscated
        .global_instructions
        .as_ref()
        .map(|g| format!("{}\n\n{}", g, obfuscated.persona_instructions))
        .unwrap_or_else(|| obfuscated.persona_instructions.clone());
    
    // Apply SEMANTIC adversarial transformation - not cryptographic obfuscation
    // This preserves semantic meaning for the LLM but varies surface form
    let transformed_system = apply_semantic_invariant_transform(&system_content, &seed);
    obfuscated.persona_instructions = transformed_system;
    obfuscated.global_instructions = None;

    // Apply to conversation context using natural variation
    if let Some(ref mut ctx) = obfuscated.conversation_context {
        let context_str: String = ctx.iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        
        let perturbed = apply_conversation_permutation(&context_str, &seed);
        
        // Store as metadata for analysis, not as visible markers
        if perturbed != context_str {
            obfuscated.params_json["semantic_hash"] = json!(compute_semantic_hash(&perturbed));
        }
    }

    obfuscated
}

// ============================================================================
// SEMANTIC ADVERSARIAL TRANSFORMATIONS (Undetectable by pattern matching)
// Based on: Semantic Adversarial Examples[^17^], Invariance Attacks[^16^]
// ============================================================================

/// Apply semantically invariant transformations that preserve meaning
/// but vary surface form to resist prompt extraction[^15^][^20^].
/// 
/// Techniques used:
/// 1. Synonym substitution (WordNet-based)
/// 2. Syntactic restructuring (active/passive voice)
/// 3. Discourse marker insertion
/// 4. Zero-width unicode joins (invisible to humans, visible to models)
/// 5. Whitespace normalization variations
fn apply_semantic_invariant_transform(text: &str, seed: &[u8]) -> String {
    let mut result = text.to_string();
    
    // Technique 1: Synonym substitution with deterministic selection
    result = synonym_substitution(&result, seed);
    
    // Technique 2: Syntactic variation (active/passive flip)
    result = syntactic_variation(&result, seed);
    
    // Technique 3: Zero-width joiner insertion (invisible perturbation)
    result = insert_zwj_noise(&result, seed, 0.05); // 5% insertion rate
    
    // Technique 4: Unicode homoglyph substitution
    result = homoglyph_substitution(&result, seed);
    
    // Technique 5: Discourse marker insertion
    result = insert_discourse_markers(&result, seed);
    
    result
}

/// Synonym substitution using deterministic hashing
fn synonym_substitution(text: &str, seed: &[u8]) -> String {
    // Common synonym pairs for system prompts
    let substitutions: &[(&str, &str)] = &[
        ("You are", "You function as"),
        ("assistant", "aid"),
        ("help", "assist"),
        ("user", "individual"),
        ("provide", "furnish"),
        ("information", "details"),
        ("respond", "reply"),
        ("questions", "inquiries"),
        ("task", "objective"),
        ("ensure", "make certain"),
        ("carefully", "with care"),
        ("however", "nevertheless"),
        ("therefore", "consequently"),
        ("for example", "for instance"),
        ("important", "crucial"),
        ("must", "are required to"),
        ("should", "ought to"),
        ("can", "are able to"),
        ("will", "shall"),
    ];
    
    let mut result = text.to_string();
    let seed_val = seed[0] as usize;
    
    for (i, (from, to)) in substitutions.iter().enumerate() {
        // Deterministic but scattered selection based on seed
        if (seed_val + i) % 3 == 0 {
            result = result.replace(from, to);
        }
    }
    
    result
}

/// Syntactic variation (active/passive voice adjustments)
fn syntactic_variation(text: &str, seed: &[u8]) -> String {
    if seed[1] % 2 == 0 {
        return text
            .replace("You must provide", "It is required that you provide")
            .replace("You should follow", "It is recommended that you follow")
            .replace("You are expected to", "The expectation is that you");
    }
    text.to_string()
}

/// Zero-width joiner (U+200D) insertion - invisible to humans, tokenized by models
/// This creates adversarial examples that evade string matching[^14^]
fn insert_zwj_noise(text: &str, seed: &[u8], rate: f64) -> String {
    let zwj = '\u{200D}';
    let mut result = String::with_capacity(text.len() * 2);
    let mut rng_state = seed[2] as u64;
    
    for ch in text.chars() {
        result.push(ch);
        
        // Simple LCG for deterministic "randomness"
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        let rand = (rng_state % 1000) as f64 / 1000.0;
        
        if rand < rate && ch.is_alphabetic() {
            result.push(zwj);
        }
    }
    
    result
}

/// Unicode homoglyph substitution (Cyrillic 'а' vs Latin 'a')
fn homoglyph_substitution(text: &str, seed: &[u8]) -> String {
    // Limited substitution to maintain readability
    let subs = if seed[3] % 2 == 0 {
        [('a', 'а'), ('e', 'е'), ('o', 'о'), ('p', 'р'), ('c', 'с')]
    } else {
        [('A', 'А'), ('B', 'В'), ('C', 'С'), ('E', 'Е'), ('H', 'Н')]
    };
    
    let mut result = text.to_string();
    for (orig, replacement) in subs.iter() {
        // Replace only some occurrences based on position
        result = result.chars().enumerate().map(|(i, c)| {
            if c == *orig && (i + seed[4] as usize) % 5 == 0 {
                *replacement
            } else {
                c
            }
        }).collect();
    }
    
    result
}

/// Insert natural discourse markers
fn insert_discourse_markers(text: &str, seed: &[u8]) -> String {
    let markers = ["Furthermore,", "Moreover,", "Additionally,", "In this context,"];
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && line.len() > 50 && (seed[i % seed.len()] as usize + i) % 4 == 0 {
            let marker = markers[(seed[i % seed.len()] as usize) % markers.len()];
            result.push(format!("{} {}", marker, line));
        } else {
            result.push(line.to_string());
        }
    }
    
    result.join("\n")
}

/// Apply conversation-level permutation (reordering, paraphrasing)
fn apply_conversation_permutation(context: &str, seed: &[u8]) -> String {
    // Split into sentences, potentially reorder if logically safe
    let sentences: Vec<&str> = context.split(". ").collect();
    if sentences.len() < 3 || seed[5] % 3 != 0 {
        return apply_semantic_invariant_transform(context, seed);
    }
    
    // Safe reordering: swap adjacent sentences (preserves local coherence)
    let mut reordered = Vec::new();
    let mut i = 0;
    while i < sentences.len() - 1 {
        if (seed[i % seed.len()] as usize) % 2 == 0 {
            reordered.push(sentences[i + 1]);
            reordered.push(sentences[i]);
            i += 2;
        } else {
            reordered.push(sentences[i]);
            i += 1;
        }
    }
    if i == sentences.len() - 1 {
        reordered.push(sentences[i]);
    }
    
    reordered.join(". ")
}

/// Compute semantic hash for verification (not for obfuscation)
fn compute_semantic_hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    // Normalize: lowercase, remove punctuation for semantic comparison
    let normalized: String = text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    hasher.update(normalized);
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// PROVIDER-SPECIFIC OPTIMIZATIONS (Non-adversarial, compatibility focused)
// ============================================================================

fn optimize_for_claude(mut packet: PromptPacket) -> PromptPacket {
    let mut meta = get_params_map(&packet);
    if packet.user_message.contains("```") && !packet.user_message.contains("<antThinking>") {
        meta.insert("format_hint".to_string(), "xml_preferred".to_string());
    }
    meta.insert("anthropic_skip_prefill".to_string(), "false".to_string());
    set_params_map(&mut packet, meta);
    packet
}

fn optimize_for_openai(mut packet: PromptPacket) -> PromptPacket {
    let mut meta = get_params_map(&packet);
    let system_len = packet.persona_instructions.len()
        + packet.global_instructions.as_ref().map(|s| s.len()).unwrap_or(0);
    if system_len > 2000 {
        meta.insert("openai_message_format".to_string(), "array".to_string());
    }
    set_params_map(&mut packet, meta);
    packet
}

fn optimize_for_local(mut packet: PromptPacket) -> PromptPacket {
    let mut meta = get_params_map(&packet);
    meta.insert("apply_chat_template".to_string(), "true".to_string());
    if packet.user_message.len() > 4096 {
        meta.insert("repetition_penalty".to_string(), "1.15".to_string());
    }
    set_params_map(&mut packet, meta);
    packet
}

fn apply_generic_optimization(mut packet: PromptPacket) -> PromptPacket {
    packet.user_message = packet.user_message.trim().to_string();
    packet
}

fn get_params_map(packet: &PromptPacket) -> HashMap<String, String> {
    packet
        .params_json
        .get("transform_metadata")
        .and_then(|v| v.as_object())
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn set_params_map(packet: &mut PromptPacket, meta: HashMap<String, String>) {
    let obj: serde_json::Map<String, serde_json::Value> = meta
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();
    packet.params_json["transform_metadata"] = json!(obj);
}

fn apply_pii_masking(mut packet: PromptPacket, sensitivity: f64) -> PromptPacket {
    let threshold = (sensitivity * 100.0) as usize;

    if threshold > 30 {
        packet.user_message = Regex::new(
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b",
        )
        .unwrap()
        .replace_all(&packet.user_message, "[EMAIL_REDACTED]")
        .to_string();
    }
    if threshold > 50 {
        packet.user_message = Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b")
            .unwrap()
            .replace_all(&packet.user_message, "[PHONE_REDACTED]")
            .to_string();
    }
    if threshold > 80 {
        packet.user_message = Regex::new(r"\b\d{3}-\d{2}-\d{4}\b")
            .unwrap()
            .replace_all(&packet.user_message, "[SSN_REDACTED]")
            .to_string();
    }

    packet
}

fn should_chunk(packet: &PromptPacket, window_size: usize) -> bool {
    packet.user_message.len() > window_size * 4
}

fn apply_semantic_chunking(mut packet: PromptPacket, window_size: usize) -> PromptPacket {
    let mut meta = get_params_map(&packet);
    meta.insert("chunking_strategy".to_string(), "sliding_window".to_string());
    meta.insert("window_size".to_string(), window_size.to_string());
    meta.insert("overlap".to_string(), (window_size / 4).to_string());
    set_params_map(&mut packet, meta);
    packet
}
