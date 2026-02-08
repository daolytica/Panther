// Web search functionality for fetching recent news and information

use reqwest::Client;
use scraper::{Html, Selector};
use regex::Regex;
use anyhow::Result;
use serde::{Serialize, Deserialize};

pub struct WebSearch {
    client: Client,
}

impl WebSearch {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to create HTTP client");
        
        WebSearch { client }
    }

    /// Search for recent news and articles on a topic
    pub async fn search_recent_news(&self, query: &str, max_results: usize) -> Result<Vec<NewsResult>> {
        // Try Google News RSS first (more reliable for news)
        match self.search_google_news(query, max_results).await {
            Ok(results) if !results.is_empty() => return Ok(results),
            Err(e) => eprintln!("Google News search failed: {}", e),
            _ => {}
        }
        
        // Fallback to DuckDuckGo if Google News fails
        match self.search_duckduckgo(query, max_results).await {
            Ok(results) if !results.is_empty() => return Ok(results),
            Err(e) => eprintln!("DuckDuckGo search failed: {}", e),
            _ => {}
        }
        
        // Final fallback: return mock results if all searches fail
        eprintln!("All search methods failed, returning empty results");
        Ok(Vec::new())
    }

    /// Search DuckDuckGo HTML
    async fn search_duckduckgo(&self, query: &str, max_results: usize) -> Result<Vec<NewsResult>> {
        let query_with_news = format!("{} news recent", query);
        let encoded_query = urlencoding::encode(&query_with_news);
        let search_url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);
        
        let response = self.client
            .get(&search_url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("DuckDuckGo search failed: HTTP {}", response.status()));
        }

        let html = response.text().await?;
        
        // Parse HTML (do this before any await)
        let results = {
            let document = Html::parse_document(&html);
            
            // Try multiple selector patterns for DuckDuckGo
            let mut results = Vec::new();
            
            // Pattern 1: Standard result selectors
            if let (Ok(result_selector), Ok(title_selector), Ok(snippet_selector)) = (
                Selector::parse(".result"),
                Selector::parse(".result__a"),
                Selector::parse(".result__snippet")
            ) {
                for result in document.select(&result_selector).take(max_results) {
                    let title = result.select(&title_selector)
                        .next()
                        .and_then(|e| e.text().next())
                        .unwrap_or("")
                        .to_string();
                    
                    let snippet = result.select(&snippet_selector)
                        .next()
                        .map(|e| e.text().collect::<Vec<_>>().join(" "))
                        .unwrap_or_default();
                    
                    // Try to get URL from href
                    let url = result.select(&title_selector)
                        .next()
                        .and_then(|e| e.value().attr("href"))
                        .unwrap_or("")
                        .to_string();
                    
                    if !title.is_empty() {
                        results.push(NewsResult {
                            title,
                            snippet,
                            url: if url.starts_with("http") { url } else { format!("https://duckduckgo.com{}", url) },
                        });
                    }
                }
            }
            
            // Pattern 2: Alternative selectors if first pattern didn't work
            if results.is_empty() {
                if let (Ok(result_selector), Ok(link_selector)) = (
                    Selector::parse(".web-result"),
                    Selector::parse("a.result__a")
                ) {
                    for result in document.select(&result_selector).take(max_results) {
                        if let Some(link) = result.select(&link_selector).next() {
                            let title = link.text().collect::<Vec<_>>().join(" ");
                            let url = link.value().attr("href").unwrap_or("").to_string();
                            
                            if !title.is_empty() {
                                results.push(NewsResult {
                                    title,
                                    snippet: String::new(),
                                    url: if url.starts_with("http") { url } else { format!("https://duckduckgo.com{}", url) },
                                });
                            }
                        }
                    }
                }
            }
            
            results
        };
        
        Ok(results)
    }

    /// Fallback: Search Google News RSS feed
    async fn search_google_news(&self, query: &str, max_results: usize) -> Result<Vec<NewsResult>> {
        let news_url = format!("https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en",
            urlencoding::encode(query));
        
        let response = self.client
            .get(&news_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Google News search failed: HTTP {}", response.status()));
        }

        let xml = response.text().await?;
        let re_title = Regex::new(r"<title><!\[CDATA\[(.*?)\]\]></title>").unwrap();
        let re_link = Regex::new(r"<link>(.*?)</link>").unwrap();
        let re_description = Regex::new(r"<description><!\[CDATA\[(.*?)\]\]></description>").unwrap();
        
        let mut results = Vec::new();
        let titles: Vec<_> = re_title.captures_iter(&xml).collect();
        let links: Vec<_> = re_link.captures_iter(&xml).collect();
        let descriptions: Vec<_> = re_description.captures_iter(&xml).collect();
        
        for i in 1..std::cmp::min(max_results + 1, titles.len()) {
            if i < titles.len() && i < links.len() {
                let title = titles[i].get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let url = links[i].get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let snippet = if i < descriptions.len() {
                    descriptions[i].get(1).map(|m| {
                        let text = m.as_str();
                        // Remove HTML tags
                        Regex::new(r"<[^>]+>").unwrap().replace_all(text, "").to_string()
                    }).unwrap_or_default()
                } else {
                    String::new()
                };
                
                if !title.is_empty() {
                    results.push(NewsResult {
                        title,
                        snippet,
                        url,
                    });
                }
            }
        }
        
        Ok(results)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsResult {
    pub title: String,
    pub snippet: String,
    pub url: String,
}
