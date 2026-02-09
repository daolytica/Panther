// Training data import commands

use crate::db::Database;
use serde::{Deserialize, Serialize};
use tauri::State;
use std::fs;
use std::io::Read;
use std::path::Path;
use reqwest;
use serde_json;
use uuid::Uuid;
use chrono::Utc;
use walkdir::WalkDir;
use regex::Regex;

// ============================================================================
// Research Paper Parsing Structures
// ============================================================================

/// Citation type detected in the paper
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CitationType {
    Numeric,      // [1], [2], [1-3]
    AuthorYear,   // (Smith, 2020), (Smith et al., 2020)
    Footnote,     // superscript numbers
    Unknown,
}

/// A citation marker found in the text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub marker: String,              // "[1]" or "(Smith, 2020)"
    pub reference_text: Option<String>, // Full reference if found in References section
    pub citation_type: CitationType,
}

/// Metadata extracted from the paper
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaperMetadata {
    pub doi: Option<String>,
    pub journal: Option<String>,
    pub year: Option<i32>,
    pub publisher: Option<String>,
    pub arxiv_id: Option<String>,
    pub keywords: Vec<String>,
}

/// A section of the paper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperSection {
    pub id: String,              // e.g., "sec_1", "sec_2_1"
    pub heading: String,
    pub level: u8,               // 1=main section, 2=subsection
    pub content: String,
    pub token_estimate: usize,   // Approximate token count
}

/// Table content extracted from the paper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableContent {
    pub id: String,
    pub caption: Option<String>,
    pub content: String,
}

/// Figure caption extracted from the paper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigureCaption {
    pub id: String,
    pub caption: String,
}

/// Complete parsed research paper content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPaperContent {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub abstract_text: Option<String>,
    pub sections: Vec<PaperSection>,
    pub unassigned_content: String,  // Text that doesn't match any section
    pub citations: Vec<Citation>,
    pub tables: Vec<TableContent>,
    pub figures: Vec<FigureCaption>,
    pub metadata: PaperMetadata,
    pub parsing_warnings: Vec<String>,
}

impl Default for ResearchPaperContent {
    fn default() -> Self {
        ResearchPaperContent {
            title: None,
            authors: Vec::new(),
            abstract_text: None,
            sections: Vec::new(),
            unassigned_content: String::new(),
            citations: Vec::new(),
            tables: Vec::new(),
            figures: Vec::new(),
            metadata: PaperMetadata::default(),
            parsing_warnings: Vec::new(),
        }
    }
}

/// Estimate token count for text (rough approximation: ~4 chars per token)
fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize
}

/// Parse research paper from extracted PDF text
/// This is a "best-effort" heuristic parser - not guaranteed to work on all layouts
pub fn parse_research_paper(text: &str) -> ResearchPaperContent {
    let mut paper = ResearchPaperContent::default();
    let lines: Vec<&str> = text.lines().collect();
    
    if lines.is_empty() {
        paper.parsing_warnings.push("Empty document".to_string());
        return paper;
    }

    // Common section headers (case-insensitive)
    let section_patterns = [
        ("abstract", "Abstract"),
        ("introduction", "Introduction"),
        ("background", "Background"),
        ("related work", "Related Work"),
        ("literature review", "Literature Review"),
        ("methodology", "Methodology"),
        ("methods", "Methods"),
        ("materials and methods", "Materials and Methods"),
        ("approach", "Approach"),
        ("proposed method", "Proposed Method"),
        ("model", "Model"),
        ("architecture", "Architecture"),
        ("experiments", "Experiments"),
        ("experimental setup", "Experimental Setup"),
        ("results", "Results"),
        ("evaluation", "Evaluation"),
        ("findings", "Findings"),
        ("discussion", "Discussion"),
        ("analysis", "Analysis"),
        ("conclusion", "Conclusion"),
        ("conclusions", "Conclusions"),
        ("future work", "Future Work"),
        ("limitations", "Limitations"),
        ("acknowledgements", "Acknowledgements"),
        ("acknowledgments", "Acknowledgments"),
        ("references", "References"),
        ("bibliography", "Bibliography"),
        ("appendix", "Appendix"),
    ];

    // Build regex for section detection
    // Match lines that look like section headers: optional number, then keyword
    let section_regex = Regex::new(
        r"(?i)^[\s\d.]*\s*(abstract|introduction|background|related\s+work|literature\s+review|methodology|methods|materials\s+and\s+methods|approach|proposed\s+method|model|architecture|experiments|experimental\s+setup|results|evaluation|findings|discussion|analysis|conclusions?|future\s+work|limitations|acknowledgements?|references|bibliography|appendix)\s*$"
    ).unwrap();

    // Subsection regex (numbered like "2.1 Subsection Title")
    let subsection_regex = Regex::new(r"^(\d+\.\d+\.?\d*)\s+(.+)$").unwrap();

    // Citation patterns
    let numeric_citation_regex = Regex::new(r"\[(\d+(?:[-–,]\s*\d+)*)\]").unwrap();
    let author_year_regex = Regex::new(r"\(([A-Z][a-z]+(?:\s+(?:et\s+al\.?|and|&)\s+[A-Z][a-z]+)?),?\s*(\d{4})\)").unwrap();

    // DOI pattern
    let doi_regex = Regex::new(r"(?i)(?:doi[:\s]*)?10\.\d{4,}/[^\s]+").unwrap();
    
    // arXiv pattern
    let arxiv_regex = Regex::new(r"(?i)arXiv[:\s]*(\d{4}\.\d{4,5})").unwrap();

    // Extract metadata from text
    if let Some(doi_match) = doi_regex.find(text) {
        let doi = doi_match.as_str().to_string();
        // Clean up DOI
        let doi = doi.trim_start_matches(|c: char| !c.is_ascii_digit()).to_string();
        paper.metadata.doi = Some(doi);
    }

    if let Some(caps) = arxiv_regex.captures(text) {
        paper.metadata.arxiv_id = caps.get(1).map(|m| m.as_str().to_string());
    }

    // Try to extract year from text (look for 4-digit years near the beginning)
    let year_regex = Regex::new(r"\b(20[0-2]\d|19\d{2})\b").unwrap();
    let first_500_chars: String = text.chars().take(500).collect();
    if let Some(caps) = year_regex.captures(&first_500_chars) {
        if let Ok(year) = caps.get(1).unwrap().as_str().parse::<i32>() {
            paper.metadata.year = Some(year);
        }
    }

    // Extract citations
    for caps in numeric_citation_regex.captures_iter(text) {
        let marker = caps.get(0).unwrap().as_str().to_string();
        if !paper.citations.iter().any(|c| c.marker == marker) {
            paper.citations.push(Citation {
                marker,
                reference_text: None,
                citation_type: CitationType::Numeric,
            });
        }
    }

    for caps in author_year_regex.captures_iter(text) {
        let marker = caps.get(0).unwrap().as_str().to_string();
        if !paper.citations.iter().any(|c| c.marker == marker) {
            paper.citations.push(Citation {
                marker,
                reference_text: None,
                citation_type: CitationType::AuthorYear,
            });
        }
    }

    // Extract table and figure captions
    let table_regex = Regex::new(r"(?i)Table\s*(\d+)[:\.\s]+(.+?)(?:\n|$)").unwrap();
    let figure_regex = Regex::new(r"(?i)(?:Figure|Fig\.?)\s*(\d+)[:\.\s]+(.+?)(?:\n|$)").unwrap();

    for caps in table_regex.captures_iter(text) {
        let id = format!("table_{}", caps.get(1).unwrap().as_str());
        let caption = caps.get(2).map(|m| m.as_str().trim().to_string());
        paper.tables.push(TableContent {
            id,
            caption,
            content: String::new(), // Table content extraction is complex
        });
    }

    for caps in figure_regex.captures_iter(text) {
        let id = format!("fig_{}", caps.get(1).unwrap().as_str());
        let caption = caps.get(2).unwrap().as_str().trim().to_string();
        paper.figures.push(FigureCaption { id, caption });
    }

    // Parse sections
    let mut current_section: Option<PaperSection> = None;
    let mut section_counter = 0;
    let mut in_abstract = false;
    let mut abstract_lines: Vec<&str> = Vec::new();
    let mut unassigned_lines: Vec<&str> = Vec::new();
    let mut title_candidate: Option<String> = None;

    // First few non-empty lines might be the title
    let mut title_lines_checked = 0;
    for line in &lines {
        let line_trimmed = line.trim();
        if line_trimmed.is_empty() {
            continue;
        }
        if title_lines_checked == 0 && line_trimmed.len() > 10 && line_trimmed.len() < 200 {
            // First substantial line is likely the title
            title_candidate = Some(line_trimmed.to_string());
        }
        title_lines_checked += 1;
        if title_lines_checked > 5 {
            break;
        }
    }
    paper.title = title_candidate;

    for line in &lines {
        let line_trimmed = line.trim();
        
        // Check if this is a section header
        if section_regex.is_match(line_trimmed) {
            // Save previous section if exists
            if let Some(mut section) = current_section.take() {
                section.content = section.content.trim().to_string();
                section.token_estimate = estimate_tokens(&section.content);
                paper.sections.push(section);
            }

            // Handle abstract specially
            if line_trimmed.to_lowercase().contains("abstract") {
                in_abstract = true;
                continue;
            } else if in_abstract {
                // End of abstract, save it
                paper.abstract_text = Some(abstract_lines.join("\n").trim().to_string());
                abstract_lines.clear();
                in_abstract = false;
            }

            // Find the canonical section name
            let section_name = section_patterns.iter()
                .find(|(pattern, _)| line_trimmed.to_lowercase().contains(pattern))
                .map(|(_, name)| name.to_string())
                .unwrap_or_else(|| line_trimmed.to_string());

            section_counter += 1;
            current_section = Some(PaperSection {
                id: format!("sec_{}", section_counter),
                heading: section_name,
                level: 1,
                content: String::new(),
                token_estimate: 0,
            });
        } else if let Some(caps) = subsection_regex.captures(line_trimmed) {
            // This is a numbered subsection
            if let Some(mut section) = current_section.take() {
                section.content = section.content.trim().to_string();
                section.token_estimate = estimate_tokens(&section.content);
                paper.sections.push(section);
            }

            let number = caps.get(1).unwrap().as_str();
            let title = caps.get(2).unwrap().as_str().trim();
            section_counter += 1;
            current_section = Some(PaperSection {
                id: format!("sec_{}", number.replace(".", "_")),
                heading: title.to_string(),
                level: 2,
                content: String::new(),
                token_estimate: 0,
            });
        } else if in_abstract {
            abstract_lines.push(line_trimmed);
        } else if let Some(ref mut section) = current_section {
            section.content.push_str(line_trimmed);
            section.content.push('\n');
        } else {
            // Content before any section
            unassigned_lines.push(line_trimmed);
        }
    }

    // Save last section
    if let Some(mut section) = current_section.take() {
        section.content = section.content.trim().to_string();
        section.token_estimate = estimate_tokens(&section.content);
        paper.sections.push(section);
    }

    // Save abstract if we were still in it
    if in_abstract && !abstract_lines.is_empty() {
        paper.abstract_text = Some(abstract_lines.join("\n").trim().to_string());
    }

    // Save unassigned content
    paper.unassigned_content = unassigned_lines.join("\n").trim().to_string();

    // Add warnings if parsing seems incomplete
    if paper.sections.is_empty() {
        paper.parsing_warnings.push("No standard sections detected. Paper structure may be non-standard.".to_string());
        // Put all content in unassigned
        paper.unassigned_content = text.to_string();
    }

    if paper.abstract_text.is_none() {
        paper.parsing_warnings.push("No abstract detected.".to_string());
    }

    paper
}

/// List PDF files in a folder (optionally recursive)
#[tauri::command]
pub async fn list_pdf_files_in_folder(
    folder_path: String,
    recursive: bool,
) -> Result<Vec<String>, String> {
    let path = Path::new(&folder_path);
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", folder_path));
    }
    let mut pdfs = Vec::new();
    if recursive {
        for entry in WalkDir::new(&folder_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "node_modules"
            })
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext.to_string_lossy().to_lowercase() == "pdf" {
                        if let Some(s) = p.to_str() {
                            pdfs.push(s.to_string());
                        }
                    }
                }
            }
        }
    } else {
        for entry in fs::read_dir(&folder_path).map_err(|e| format!("Failed to read folder: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext.to_string_lossy().to_lowercase() == "pdf" {
                        if let Some(s) = p.to_str() {
                            pdfs.push(s.to_string());
                        }
                    }
                }
            }
        }
    }
    pdfs.sort();
    Ok(pdfs)
}

/// Parse PDF and return structured research paper content
#[tauri::command]
pub async fn parse_pdf_as_research_paper(
    file_path: String,
) -> Result<ResearchPaperContent, String> {
    let text = extract_text_from_pdf(&file_path)?;
    Ok(parse_research_paper(&text))
}

/// Import research paper with section selection
#[derive(Debug, Serialize, Deserialize)]
pub struct ResearchPaperImportRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub file_path: String,
    pub include_sections: Vec<String>,  // Section IDs to include, empty = all
    pub include_abstract: bool,
    pub include_unassigned: bool,
    pub chunk_by_section: bool,  // If true, create one training example per section
}

#[tauri::command]
pub async fn import_research_paper(
    db: State<'_, Database>,
    request: ResearchPaperImportRequest,
) -> Result<ImportResult, String> {
    let text = extract_text_from_pdf(&request.file_path)?;
    let paper = parse_research_paper(&text);
    
    let mut success_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();
    
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let now = Utc::now().to_rfc3339();
    
    // Build content to import based on selections
    if request.chunk_by_section {
        // Create one training example per selected section
        if request.include_abstract {
            if let Some(ref abstract_text) = paper.abstract_text {
                let id = Uuid::new_v4().to_string();
                let metadata = serde_json::json!({
                    "source": "research_paper",
                    "section": "abstract",
                    "file": request.file_path,
                    "title": paper.title,
                });
                
                match conn_guard.execute(
                    "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        id,
                        request.project_id,
                        request.local_model_id,
                        "Summarize the following research paper abstract:",
                        abstract_text,
                        serde_json::to_string(&metadata).unwrap_or_default(),
                        now
                    ],
                ) {
                    Ok(_) => success_count += 1,
                    Err(e) => {
                        error_count += 1;
                        errors.push(format!("Failed to import abstract: {}", e));
                    }
                }
            }
        }
        
        for section in &paper.sections {
            // Skip if not in include list (unless include list is empty = include all)
            if !request.include_sections.is_empty() && !request.include_sections.contains(&section.id) {
                continue;
            }
            
            let id = Uuid::new_v4().to_string();
            let metadata = serde_json::json!({
                "source": "research_paper",
                "section": section.heading,
                "section_id": section.id,
                "file": request.file_path,
                "title": paper.title,
            });
            
            let input = format!("Explain the {} section of this research paper:", section.heading);
            
            match conn_guard.execute(
                "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    request.project_id,
                    request.local_model_id,
                    input,
                    section.content,
                    serde_json::to_string(&metadata).unwrap_or_default(),
                    now
                ],
            ) {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error_count += 1;
                    errors.push(format!("Failed to import section '{}': {}", section.heading, e));
                }
            }
        }
        
        if request.include_unassigned && !paper.unassigned_content.is_empty() {
            let id = Uuid::new_v4().to_string();
            let metadata = serde_json::json!({
                "source": "research_paper",
                "section": "unassigned",
                "file": request.file_path,
                "title": paper.title,
            });
            
            match conn_guard.execute(
                "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    request.project_id,
                    request.local_model_id,
                    "Provide additional context from this research paper:",
                    paper.unassigned_content,
                    serde_json::to_string(&metadata).unwrap_or_default(),
                    now
                ],
            ) {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error_count += 1;
                    errors.push(format!("Failed to import unassigned content: {}", e));
                }
            }
        }
    } else {
        // Combine selected sections into one training example
        let mut combined_content = String::new();
        
        if request.include_abstract {
            if let Some(ref abstract_text) = paper.abstract_text {
                combined_content.push_str("## Abstract\n\n");
                combined_content.push_str(abstract_text);
                combined_content.push_str("\n\n");
            }
        }
        
        for section in &paper.sections {
            if !request.include_sections.is_empty() && !request.include_sections.contains(&section.id) {
                continue;
            }
            combined_content.push_str(&format!("## {}\n\n", section.heading));
            combined_content.push_str(&section.content);
            combined_content.push_str("\n\n");
        }
        
        if request.include_unassigned && !paper.unassigned_content.is_empty() {
            combined_content.push_str("## Additional Content\n\n");
            combined_content.push_str(&paper.unassigned_content);
        }
        
        if !combined_content.trim().is_empty() {
            let id = Uuid::new_v4().to_string();
            let title = paper.title.clone().unwrap_or_else(|| "Research Paper".to_string());
            let metadata = serde_json::json!({
                "source": "research_paper",
                "file": request.file_path,
                "title": title,
                "sections_included": request.include_sections,
            });
            
            match conn_guard.execute(
                "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    request.project_id,
                    request.local_model_id,
                    format!("Explain the key findings and methodology of the paper: {}", title),
                    combined_content.trim(),
                    serde_json::to_string(&metadata).unwrap_or_default(),
                    now
                ],
            ) {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error_count += 1;
                    errors.push(format!("Failed to import paper: {}", e));
                }
            }
        }
    }
    
    Ok(ImportResult {
        success_count,
        error_count,
        errors,
    })
}

// ============================================================================
// Original Import Structures
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportTrainingDataRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub source_type: String, // "local_file", "url", "cloud"
    pub source_path: String, // file path, URL, or cloud identifier
    pub format: String, // "json", "csv", "jsonl", "txt", "auto"
    pub mapping: Option<serde_json::Value>, // field mapping for CSV
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub success_count: i32,
    pub error_count: i32,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn import_training_data_from_file(
    db: State<'_, Database>,
    request: ImportTrainingDataRequest,
) -> Result<ImportResult, String> {
    // Auto-detect format if needed
    let mut request_with_format = request;
    if request_with_format.format == "auto" {
        // Detect from file extension
        request_with_format.format = detect_format_from_path(&request_with_format.source_path);
    }
    
    // Extract text from file (handles both text and binary formats like PDF, DOCX, etc.)
    let content = extract_text_from_file(&request_with_format.source_path)?;
    
    // If format was auto-detected, try to refine it based on content
    if request_with_format.format == "auto" || request_with_format.format == "txt" {
        let detected = detect_format_from_content(&content);
        // If detected as JSONL, verify it's actually valid JSONL
        if detected == "jsonl" {
            let first_lines: Vec<&str> = content.lines().take(3).collect();
            let mut is_valid_jsonl = false;
            for line in &first_lines {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if serde_json::from_str::<serde_json::Value>(line).is_ok() {
                    is_valid_jsonl = true;
                    break;
                }
            }
            if is_valid_jsonl {
                request_with_format.format = detected;
            }
        } else if detected != "txt" {
            request_with_format.format = detected;
        }
    }
    
    parse_and_import(&db, &request_with_format, &content).await
}

#[tauri::command]
pub async fn import_training_data_from_url(
    db: State<'_, Database>,
    request: ImportTrainingDataRequest,
) -> Result<ImportResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .get(&request.source_path)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }
    
    let content = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    parse_and_import(&db, &request, &content).await
}

#[tauri::command]
pub async fn import_training_data_from_text(
    db: State<'_, Database>,
    project_id: String,
    local_model_id: Option<String>,
    content: String,
    format: String,
) -> Result<ImportResult, String> {
    let request = ImportTrainingDataRequest {
        project_id,
        local_model_id,
        source_type: "text".to_string(),
        source_path: String::new(),
        format,
        mapping: None,
    };
    
    // Auto-detect format if needed
    let format = if request.format == "auto" {
        detect_format_from_path(&request.source_path)
    } else {
        request.format.clone()
    };
    
    let mut request_with_format = request.clone();
    request_with_format.format = format;
    
    // Use the content parameter directly (it's already text)
    parse_and_import(&db, &request_with_format, &content).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportFolderRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub folder_path: String,
    pub include_subfolders: bool,
}

#[tauri::command]
pub async fn import_training_data_from_folder(
    db: State<'_, Database>,
    request: ImportFolderRequest,
) -> Result<ImportResult, String> {
    let mut success_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();
    
    // Supported file extensions (documents + code)
    let supported_extensions = vec![
        "json", "jsonl", "csv", "txt", "md",
        "pdf", "doc", "docx", "rtf", "odt",
        "py", "js", "ts", "tsx", "jsx", "rs", "go", "java", "kt", "swift",
        "c", "cpp", "h", "hpp", "cs", "rb", "php", "sh", "bash", "sql",
        "yaml", "yml", "toml", "ini", "xml", "html", "css", "scss", "vue", "svelte",
    ];
    
    // Walk directory
    let walker = if request.include_subfolders {
        WalkDir::new(&request.folder_path)
    } else {
        WalkDir::new(&request.folder_path).max_depth(1)
    };
    
    for entry in walker.into_iter() {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        
        // Skip directories
        if path.is_dir() {
            continue;
        }
        
        // Check if file extension is supported
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if !supported_extensions.contains(&ext_lower.as_str()) {
                continue;
            }
        } else {
            continue;
        }
        
        // Detect format
        let format = detect_format_from_path(path.to_str().unwrap_or(""));
        
        // Extract text from file
        let content = match extract_text_from_file(path.to_str().unwrap_or("")) {
            Ok(text) => text,
            Err(e) => {
                error_count += 1;
                errors.push(format!("Failed to extract text from {}: {}", path.display(), e));
                continue;
            }
        };
        
        // Parse and import
        let import_request = ImportTrainingDataRequest {
            project_id: request.project_id.clone(),
            local_model_id: request.local_model_id.clone(),
            source_type: "local_file".to_string(),
            source_path: path.to_string_lossy().to_string(),
            format,
            mapping: None,
        };
        
        match parse_and_import(&db, &import_request, &content).await {
            Ok(result) => {
                success_count += result.success_count;
                error_count += result.error_count;
                errors.extend(result.errors);
            }
            Err(e) => {
                error_count += 1;
                errors.push(format!("Failed to import {}: {}", path.display(), e));
            }
        }
    }
    
    Ok(ImportResult {
        success_count,
        error_count,
        errors: errors.into_iter().take(20).collect(), // Limit to 20 errors
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportCoderHistoryRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub workspace_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportChatMessagesRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    /// If Some, import only from this profile. If None, import from all profiles.
    pub profile_id: Option<String>,
}

/// Import profile chat conversations (chat_messages table) as training data.
/// Pairs user messages with assistant responses.
#[tauri::command]
pub async fn import_training_data_from_chat_messages(
    db: State<'_, Database>,
    request: ImportChatMessagesRequest,
) -> Result<ImportResult, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    type RowTuple = (String, String, String, Option<String>);
    let rows: Vec<RowTuple> = if let Some(ref profile_id) = request.profile_id {
        let mut stmt = conn_guard
            .prepare("SELECT id, role, content, profile_id FROM chat_messages WHERE profile_id = ?1 ORDER BY created_at ASC")
            .map_err(|e| format!("Database error: {}", e))?;
        let mapped = stmt.query_map(rusqlite::params![profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }).map_err(|e| format!("Database error: {}", e))?;
        mapped.filter_map(|r| r.ok()).collect()
    } else {
        let mut stmt = conn_guard
            .prepare("SELECT id, role, content, profile_id FROM chat_messages ORDER BY profile_id, created_at ASC")
            .map_err(|e| format!("Database error: {}", e))?;
        let mapped = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }).map_err(|e| format!("Database error: {}", e))?;
        mapped.filter_map(|r| r.ok()).collect()
    };

    let mut success_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();
    let mut current_user_message: Option<String> = None;
    let mut last_profile_id: Option<String> = None;

    for (id, role, content, profile_id) in rows {
        // Reset pairing when switching profiles (for "all profiles" query)
        if request.profile_id.is_none() {
            if last_profile_id.as_ref() != profile_id.as_ref() {
                current_user_message = None;
                last_profile_id = profile_id.clone();
            }
        }

        match role.as_str() {
            "user" => {
                current_user_message = Some(content);
            }
            "assistant" => {
                if let Some(user_msg) = current_user_message.take() {
                    if !user_msg.trim().is_empty() || !content.trim().is_empty() {
                        let source_path = format!("chat_messages:{}", id);
                        match create_training_data_from_import(
                            &db,
                            &request.project_id,
                            request.local_model_id.as_deref(),
                            &user_msg,
                            &content,
                            "profile_chat",
                            &source_path,
                        ) {
                            Ok(_) => success_count += 1,
                            Err(e) => {
                                error_count += 1;
                                errors.push(format!("Failed to import message {}: {}", id, e));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if success_count > 0 {
        let cache = crate::cache::training_cache::TrainingCache::new((*db).clone());
        let _ = cache.invalidate_cache(&request.project_id, request.local_model_id.as_deref());
    }

    Ok(ImportResult {
        success_count,
        error_count,
        errors: errors.into_iter().take(20).collect(),
    })
}

/// Scan panther_chat_history/*.md in workspace, parse JSON blocks, extract Q&A pairs.
#[tauri::command]
pub async fn import_training_data_from_coder_history(
    db: State<'_, Database>,
    request: ImportCoderHistoryRequest,
) -> Result<ImportResult, String> {
    let history_dir = Path::new(&request.workspace_path).join("panther_chat_history");
    if !history_dir.exists() {
        return Ok(ImportResult {
            success_count: 0,
            error_count: 0,
            errors: vec!["panther_chat_history folder not found in workspace".to_string()],
        });
    }

    let mut success_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();

    let entries = fs::read_dir(&history_dir)
        .map_err(|e| format!("Failed to read panther_chat_history: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.to_lowercase() != "md" {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                error_count += 1;
                errors.push(format!("Failed to read {}: {}", path.display(), e));
                continue;
            }
        };

        // Extract JSON block: ```json ... ```
        let json_block = extract_json_block_from_markdown(&content);
        let json_block = match json_block {
            Some(b) => b,
            None => {
                error_count += 1;
                errors.push(format!("No JSON block found in {}", path.display()));
                continue;
            }
        };

        let messages: Vec<serde_json::Value> = match serde_json::from_str(&json_block) {
            Ok(m) => m,
            Err(e) => {
                error_count += 1;
                errors.push(format!("Invalid JSON in {}: {}", path.display(), e));
                continue;
            }
        };

        let mut current_user: Option<String> = None;
        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();

            match role {
                "user" => current_user = Some(content),
                "assistant" => {
                    if let Some(user_text) = current_user.take() {
                        if !user_text.trim().is_empty() || !content.trim().is_empty() {
                            match create_training_data_from_import(
                                &db,
                                &request.project_id,
                                request.local_model_id.as_deref(),
                                &user_text,
                                &content,
                                "coder_history",
                                &path.display().to_string(),
                            ) {
                                Ok(_) => success_count += 1,
                                Err(e) => {
                                    error_count += 1;
                                    errors.push(format!("Failed to import from {}: {}", path.display(), e));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if success_count > 0 {
        if let Some(ref mid) = request.local_model_id {
            let cache = crate::cache::training_cache::TrainingCache::new((*db).clone());
            let _ = cache.invalidate_cache(&request.project_id, Some(mid));
        } else {
            let cache = crate::cache::training_cache::TrainingCache::new((*db).clone());
            let _ = cache.invalidate_cache(&request.project_id, None);
        }
    }

    Ok(ImportResult {
        success_count,
        error_count,
        errors: errors.into_iter().take(20).collect(),
    })
}

fn extract_json_block_from_markdown(content: &str) -> Option<String> {
    let start = content.find("```json")?;
    let after_start = &content[start + 7..];
    let end = after_start.find("```")?;
    Some(after_start[..end].trim().to_string())
}

fn create_training_data_from_import(
    db: &Database,
    project_id: &str,
    local_model_id: Option<&str>,
    input_text: &str,
    output_text: &str,
    source: &str,
    source_path: &str,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let metadata = serde_json::json!({
        "source": source,
        "source_path": source_path,
        "imported_at": now,
    });
    let metadata_str = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());

    conn_guard
        .execute(
            "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                project_id,
                local_model_id,
                input_text,
                output_text,
                metadata_str,
                now
            ],
        )
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(())
}

fn detect_format_from_path(path: &str) -> String {
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".jsonl") {
        "jsonl".to_string()
    } else if path_lower.ends_with(".csv") {
        "csv".to_string()
    } else if path_lower.ends_with(".txt") || path_lower.ends_with(".md") {
        "txt".to_string()
    } else if path_lower.ends_with(".json") {
        "json".to_string()
    } else if path_lower.ends_with(".pdf") {
        "pdf".to_string()
    } else if path_lower.ends_with(".docx") {
        "docx".to_string()
    } else if path_lower.ends_with(".doc") {
        "doc".to_string()
    } else if path_lower.ends_with(".rtf") {
        "rtf".to_string()
    } else if path_lower.ends_with(".odt") {
        "odt".to_string()
    } else {
        // Code and other text-based formats: treat as plain text (UTF-8)
        "txt".to_string()
    }
}

fn detect_format_from_content(content: &str) -> String {
    // Try to detect format by attempting to parse content
    let trimmed = content.trim();
    
    // Check if it's JSON (starts with { or [)
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(_) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return "json".to_string();
        }
    }
    
    // Check if it's JSONL (each line is a JSON object)
    let lines: Vec<&str> = content.lines().take(5).collect();
    let mut jsonl_count = 0;
    for line in &lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('{') && serde_json::from_str::<serde_json::Value>(line).is_ok() {
            jsonl_count += 1;
        }
    }
    if jsonl_count >= 2 || (jsonl_count == 1 && lines.len() > 1) {
        return "jsonl".to_string();
    }
    
    // Check if it's CSV (has commas and potential header)
    if trimmed.contains(',') {
        let first_line = lines.first().unwrap_or(&"");
        let comma_count = first_line.matches(',').count();
        if comma_count >= 1 {
            return "csv".to_string();
        }
    }
    
    // Default to text
    "txt".to_string()
}

fn extract_text_from_file(path: &str) -> Result<String, String> {
    let path_lower = path.to_lowercase();
    
    if path_lower.ends_with(".pdf") {
        extract_text_from_pdf(path)
    } else if path_lower.ends_with(".docx") {
        extract_text_from_docx(path)
    } else if path_lower.ends_with(".doc") {
        extract_text_from_doc(path)
    } else if path_lower.ends_with(".rtf") {
        extract_text_from_rtf(path)
    } else if path_lower.ends_with(".odt") {
        extract_text_from_odt(path)
    } else {
        // For text-based formats, just read as string
        fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))
    }
}

fn extract_text_from_pdf(path: &str) -> Result<String, String> {
    use lopdf::Document;
    
    let doc = Document::load(path)
        .map_err(|e| format!("Failed to load PDF: {}", e))?;
    
    let mut text_parts = Vec::new();
    let pages = doc.get_pages();
    
    // Extract text from all pages
    for (page_num, _) in pages.iter() {
        if let Ok(page_text) = doc.extract_text(&[*page_num]) {
            if !page_text.trim().is_empty() {
                text_parts.push(page_text);
            }
        }
    }
    
    if text_parts.is_empty() {
        return Err("No text content found in PDF. The PDF may be image-based or encrypted.".to_string());
    }
    
    Ok(text_parts.join("\n\n"))
}

fn extract_text_from_docx(path: &str) -> Result<String, String> {
    use zip::ZipArchive;
    use quick_xml::events::Event;
    use quick_xml::Reader;
    
    // DOCX is a ZIP file containing XML
    let file = fs::File::open(path)
        .map_err(|e| format!("Failed to open DOCX file: {}", e))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read DOCX as ZIP: {}", e))?;
    
    let mut text_parts = Vec::new();
    
    // Extract text from document.xml
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry: {}", e))?;
        
            if file.name() == "word/document.xml" || file.name().ends_with("/word/document.xml") {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| format!("Failed to read document.xml: {}", e))?;
            
            // Parse XML and extract text from <w:t> elements (Word text nodes)
            let mut reader = Reader::from_str(&content);
            reader.trim_text(true);
            
            let mut buf = Vec::new();
            let mut in_text_element = false;
            
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(e)) => {
                        if e.name().as_ref() == b"w:t" {
                            in_text_element = true;
                        }
                    }
                    Ok(Event::Text(e)) => {
                        if in_text_element {
                            let text = e.unescape().unwrap_or_default();
                            if !text.trim().is_empty() {
                                text_parts.push(text.to_string());
                            }
                        }
                    }
                    Ok(Event::End(e)) => {
                        if e.name().as_ref() == b"w:t" {
                            in_text_element = false;
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(e) => {
                        eprintln!("XML parsing error: {}", e);
                        break;
                    }
                    _ => {}
                }
                buf.clear();
            }
            break;
        }
    }
    
    if text_parts.is_empty() {
        return Err("No text content found in DOCX".to_string());
    }
    
    Ok(text_parts.join(" "))
}

fn extract_text_from_doc(_path: &str) -> Result<String, String> {
    // DOC files are binary format, would need external tool or library
    Err("DOC text extraction not yet implemented. Please convert DOC to DOCX or text first.".to_string())
}

fn extract_text_from_rtf(path: &str) -> Result<String, String> {
    // RTF is text-based but has formatting codes
    // For now, try to read as text and strip RTF codes
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read RTF file: {}", e))?;
    
    // Simple RTF text extraction - remove RTF control codes
    let text = content
        .lines()
        .filter(|line| !line.trim().starts_with("\\"))
        .collect::<Vec<_>>()
        .join("\n");
    
    Ok(text)
}

fn extract_text_from_odt(_path: &str) -> Result<String, String> {
    // ODT is a ZIP file containing XML
    // For now, return placeholder
    Err("ODT text extraction not yet implemented. Please convert ODT to text first.".to_string())
}

async fn parse_and_import(
    db: &Database,
    request: &ImportTrainingDataRequest,
    content: &str,
) -> Result<ImportResult, String> {
    let mut success_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();
    
    let training_data = match request.format.as_str() {
        "json" => parse_json_format(content)?,
        "jsonl" => parse_jsonl_format(content)?,
        "csv" => parse_csv_format(content, &request.mapping)?,
        "txt" | "md" => {
            parse_text_as_training_data(content)?
        },
        "pdf" | "docx" | "doc" | "rtf" | "odt" => {
            // For extracted text from binary formats, treat as plain text
            parse_text_as_training_data(content)?
        },
        _ => return Err(format!("Unsupported format: {}", request.format)),
    };
    
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    for (input, output, metadata) in training_data {
        match create_training_data_entry(
            &conn_guard,
            &request.project_id,
            request.local_model_id.as_ref(),
            &input,
            &output,
            metadata.as_ref(),
        ) {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_count += 1;
                errors.push(format!("Failed to import: {}", e));
            }
        }
    }
    
    // Invalidate cache after importing training data
    if success_count > 0 {
        use crate::cache::training_cache::TrainingCache;
        let cache = TrainingCache::new(db.clone());
        if let Some(model_id) = &request.local_model_id {
            cache.invalidate_cache(&request.project_id, Some(model_id)).ok();
        } else {
            cache.invalidate_cache(&request.project_id, None).ok();
        }
    }
    
    Ok(ImportResult {
        success_count,
        error_count,
        errors: errors.into_iter().take(10).collect(), // Limit to 10 errors
    })
}

fn parse_json_format(content: &str) -> Result<Vec<(String, String, Option<serde_json::Value>)>, String> {
    let json: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("Invalid JSON: {}", e))?;
    
    let mut results = Vec::new();
    
    // Try array of objects
    if let Some(array) = json.as_array() {
        for item in array {
            if let (Some(input), Some(output)) = (
                item.get("input").or(item.get("input_text")).or(item.get("prompt")).and_then(|v| v.as_str()),
                item.get("output").or(item.get("output_text")).or(item.get("completion")).and_then(|v| v.as_str()),
            ) {
                let metadata = item.get("metadata").cloned();
                results.push((input.to_string(), output.to_string(), metadata));
            }
        }
    } else if let (Some(input), Some(output)) = (
        json.get("input").or(json.get("input_text")).or(json.get("prompt")).and_then(|v| v.as_str()),
        json.get("output").or(json.get("output_text")).or(json.get("completion")).and_then(|v| v.as_str()),
    ) {
        // Single object
        let metadata = json.get("metadata").cloned();
        results.push((input.to_string(), output.to_string(), metadata));
    }
    
    if results.is_empty() {
        return Err("No valid input/output pairs found in JSON".to_string());
    }
    
    Ok(results)
}

fn parse_jsonl_format(content: &str) -> Result<Vec<(String, String, Option<serde_json::Value>)>, String> {
    let mut results = Vec::new();
    let mut line_number = 0;
    let mut skipped_count = 0;
    
    // Remove BOM if present
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    
    for line in content.lines() {
        line_number += 1;
        let line = line.trim();
        
        // Skip empty or whitespace-only lines
        if line.is_empty() {
            continue;
        }
        
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(json) => {
                // Try multiple field name variations
                let input_opt = json.get("input")
                    .or(json.get("input_text"))
                    .or(json.get("prompt"))
                    .or(json.get("question"))
                    .or(json.get("instruction"));
                
                let output_opt = json.get("output")
                    .or(json.get("output_text"))
                    .or(json.get("completion"))
                    .or(json.get("response"))
                    .or(json.get("answer"));
                
                // Convert to string, handling both string and number types
                let input = input_opt.and_then(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if let Some(n) = v.as_number() {
                        Some(n.to_string())
                    } else {
                        None
                    }
                });
                
                let output = output_opt.and_then(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if let Some(n) = v.as_number() {
                        Some(n.to_string())
                    } else {
                        None
                    }
                });
                
                if let (Some(input), Some(output)) = (input, output) {
                    let metadata = json.get("metadata").cloned();
                    results.push((input, output, metadata));
                } else {
                    skipped_count += 1;
                    eprintln!("Skipping JSONL line {}: missing 'input' or 'output' field. Available fields: {:?}", 
                        line_number, 
                        json.as_object().map(|o| o.keys().collect::<Vec<_>>())
                    );
                }
            }
            Err(_e) => {
                skipped_count += 1;
                // Only log for lines that strongly look like they were meant to be JSON
                // A proper JSON object line MUST start with '{' or '['
                let is_json_object = line.starts_with('{') || line.starts_with('[');
                
                // Skip logging for:
                // - Plain text content (descriptions, markdown, HTML)
                // - Lines starting with special characters (-, <, #, etc.)
                // - Short lines that don't look like JSON
                if is_json_object && line.len() > 5 {
                    // This actually looks like a malformed JSON object, log it
                    eprintln!("Skipping malformed JSON on line {}: {}", 
                        line_number,
                        line.chars().take(50).collect::<String>()
                    );
                }
                // Don't log anything for plain text, markdown, HTML, or other non-JSON content
                // These are expected when the file has mixed content
            }
        }
    }
    
    if results.is_empty() {
        return Err(format!(
            "No valid input/output pairs found in JSONL. Processed {} lines, skipped {} invalid lines. \
            Expected format: {{\"input\": \"...\", \"output\": \"...\"}} or {{\"input_text\": \"...\", \"output_text\": \"...\"}}",
            line_number,
            skipped_count
        ));
    }
    
    if skipped_count > 0 {
        eprintln!("Warning: Skipped {} invalid or incomplete lines out of {} total lines", skipped_count, line_number);
    }
    
    Ok(results)
}

fn parse_csv_format(content: &str, mapping: &Option<serde_json::Value>) -> Result<Vec<(String, String, Option<serde_json::Value>)>, String> {
    let mut results = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    if lines.is_empty() {
        return Err("CSV file is empty".to_string());
    }
    
    // Parse header
    let header: Vec<&str> = lines[0].split(',').map(|s| s.trim()).collect();
    
    // Determine input/output columns
    let input_col = mapping
        .as_ref()
        .and_then(|m| m.get("input_column").and_then(|v| v.as_str()))
        .or_else(|| header.iter().find(|&h| h.eq_ignore_ascii_case("input") || h.eq_ignore_ascii_case("input_text") || h.eq_ignore_ascii_case("prompt")).copied())
        .ok_or_else(|| "Could not find input column in CSV".to_string())?;
    
    let output_col = mapping
        .as_ref()
        .and_then(|m| m.get("output_column").and_then(|v| v.as_str()))
        .or_else(|| header.iter().find(|&h| h.eq_ignore_ascii_case("output") || h.eq_ignore_ascii_case("output_text") || h.eq_ignore_ascii_case("completion")).copied())
        .ok_or_else(|| "Could not find output column in CSV".to_string())?;
    
    let input_idx = header.iter().position(|&h| h == input_col)
        .ok_or_else(|| format!("Input column '{}' not found in header", input_col))?;
    let output_idx = header.iter().position(|&h| h == output_col)
        .ok_or_else(|| format!("Output column '{}' not found in header", output_col))?;
    
    // Parse data rows
    for line in lines.iter().skip(1) {
        let fields: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
        
        if fields.len() > input_idx.max(output_idx) {
            let input = fields[input_idx].to_string();
            let output = fields[output_idx].to_string();
            
            if !input.is_empty() && !output.is_empty() {
                results.push((input, output, None));
            }
        }
    }
    
    if results.is_empty() {
        return Err("No valid data rows found in CSV".to_string());
    }
    
    Ok(results)
}

#[allow(dead_code)]
fn parse_txt_format(content: &str) -> Result<Vec<(String, String, Option<serde_json::Value>)>, String> {
    let mut results = Vec::new();
    
    // Try to parse as input/output pairs separated by blank lines or markers
    let sections: Vec<&str> = content.split("\n\n").collect();
    
    for section in sections {
        let lines: Vec<&str> = section.lines().collect();
        if lines.len() >= 2 {
            // Assume first line is input, rest is output
            let input = lines[0].trim().to_string();
            let output = lines[1..].join("\n").trim().to_string();
            
            if !input.is_empty() && !output.is_empty() {
                results.push((input, output, None));
            }
        } else if lines.len() == 1 {
            // Try to split by common separators
            let line = lines[0];
            if let Some(pos) = line.find(" -> ") {
                let input = line[..pos].trim().to_string();
                let output = line[pos + 4..].trim().to_string();
                if !input.is_empty() && !output.is_empty() {
                    results.push((input, output, None));
                }
            } else if let Some(pos) = line.find(" | ") {
                let input = line[..pos].trim().to_string();
                let output = line[pos + 3..].trim().to_string();
                if !input.is_empty() && !output.is_empty() {
                    results.push((input, output, None));
                }
            }
        }
    }
    
    if results.is_empty() {
        return Err("Could not parse text format. Expected input/output pairs separated by blank lines or ' -> ' or ' | '".to_string());
    }
    
    Ok(results)
}

fn parse_text_as_training_data(content: &str) -> Result<Vec<(String, String, Option<serde_json::Value>)>, String> {
    let mut results = Vec::new();
    
    // Try structured formats first
    if let Ok(json_results) = parse_json_format(content) {
        if !json_results.is_empty() {
            return Ok(json_results);
        }
    }
    
    if let Ok(jsonl_results) = parse_jsonl_format(content) {
        if !jsonl_results.is_empty() {
            return Ok(jsonl_results);
        }
    }
    
    // If structured parsing fails, treat as plain text
    // Split by paragraphs or sections and create input/output pairs
    let paragraphs: Vec<&str> = content
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect();
    
    if paragraphs.len() >= 2 {
        // Pair consecutive paragraphs as input/output
        for i in (0..paragraphs.len() - 1).step_by(2) {
            let input = paragraphs[i].trim().to_string();
            let output = paragraphs[i + 1].trim().to_string();
            
            if !input.is_empty() && !output.is_empty() {
                results.push((input, output, None));
            }
        }
    } else if paragraphs.len() == 1 {
        // Single paragraph - try to split by common separators
        let text = paragraphs[0];
        if let Some(pos) = text.find(" -> ") {
            let input = text[..pos].trim().to_string();
            let output = text[pos + 4..].trim().to_string();
            if !input.is_empty() && !output.is_empty() {
                results.push((input, output, None));
            }
        } else {
            // If no separator, use the whole text as both input and output
            // (for documents that are just content)
            let text_clean = text.trim().to_string();
            if !text_clean.is_empty() {
                results.push((text_clean.clone(), text_clean, None));
            }
        }
    }
    
    if results.is_empty() {
        return Err("Could not extract training data from text content".to_string());
    }
    
    Ok(results)
}

fn create_training_data_entry(
    conn: &rusqlite::Connection,
    project_id: &str,
    local_model_id: Option<&String>,
    input: &str,
    output: &str,
    metadata: Option<&serde_json::Value>,
) -> Result<String, String> {
    use uuid::Uuid;
    use chrono::Utc;
    
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let metadata_json = metadata
        .map(|v| serde_json::to_string(v).unwrap_or_default())
        .unwrap_or_default();
    
    conn.execute(
        "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, project_id, local_model_id, input, output, metadata_json, now],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(id)
}
