//! Data structures for the Kashshaf text reuse detection pipeline.

use serde::{Deserialize, Serialize};

/// A single page's lemma sequence
#[derive(Debug, Clone)]
pub struct PageLemmas {
    pub part_index: u32,
    pub page_id: u32,
    pub lemma_ids: Vec<u32>,
}

/// A single page's token sequence (includes token_ids, lemma_ids, and root_ids)
#[derive(Debug, Clone)]
pub struct PageTokens {
    pub part_index: u32,
    pub page_id: u32,
    pub token_ids: Vec<u32>,  // Original token_definition IDs (for surface form lookup)
    pub lemma_ids: Vec<u32>,  // Mapped lemma IDs (for comparison)
    pub root_ids: Vec<u32>,   // Mapped root IDs (for root-based matching, 0 = no root)
}

/// Complete token stream for a book (includes both token_ids and lemma_ids)
#[derive(Debug, Clone)]
pub struct BookTokenStream {
    pub book_id: u32,
    pub total_tokens: usize,
    pub pages: Vec<PageTokens>,
}

impl BookTokenStream {
    /// Get flat array of all token IDs in order
    pub fn flat_token_ids(&self) -> Vec<u32> {
        self.pages
            .iter()
            .flat_map(|p| p.token_ids.iter().copied())
            .collect()
    }

    /// Get flat array of all lemma IDs in order
    pub fn flat_lemma_ids(&self) -> Vec<u32> {
        self.pages
            .iter()
            .flat_map(|p| p.lemma_ids.iter().copied())
            .collect()
    }

    /// Get flat array of all root IDs in order
    pub fn flat_root_ids(&self) -> Vec<u32> {
        self.pages
            .iter()
            .flat_map(|p| p.root_ids.iter().copied())
            .collect()
    }

    /// Get the number of pages
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Get surface text with context before and after
    pub fn get_surface_text_with_context(
        &self,
        global_start: usize,
        global_end: usize,
        context_tokens: usize,
        token_to_surface: &[String],
    ) -> PassageText {
        let token_ids = self.flat_token_ids();
        let len = token_ids.len();

        let context_start = global_start.saturating_sub(context_tokens);
        let context_end = (global_end + context_tokens).min(len);

        let get_text = |start: usize, end: usize| -> String {
            if start >= end || start >= len {
                return String::new();
            }
            let actual_end = end.min(len);
            token_ids[start..actual_end]
                .iter()
                .filter_map(|&tid| {
                    if (tid as usize) < token_to_surface.len() {
                        Some(token_to_surface[tid as usize].as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        };

        PassageText {
            before: get_text(context_start, global_start),
            matched: get_text(global_start, global_end),
            after: get_text(global_end, context_end),
        }
    }
}

/// Reconstructed text for a passage with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassageText {
    pub before: String,   // Context before match
    pub matched: String,  // The matched text
    pub after: String,    // Context after match
}

/// Complete lemma stream for a book
#[derive(Debug, Clone)]
pub struct BookLemmaStream {
    pub book_id: u32,
    pub total_tokens: usize,
    pub pages: Vec<PageLemmas>,
}

impl BookLemmaStream {
    /// Get flat array of all lemma IDs in order
    pub fn flat_lemmas(&self) -> Vec<u32> {
        self.pages
            .iter()
            .flat_map(|p| p.lemma_ids.iter().copied())
            .collect()
    }

    /// Get the number of pages
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

/// A window into a book's lemma/root stream
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Window {
    pub book_id: u32,
    pub window_idx: u32,
    pub global_start: usize,
    pub global_end: usize,
    pub start_page: (u32, u32), // (part_index, page_id)
    pub start_offset: u32,      // Offset within start page
    pub end_page: (u32, u32),
    pub end_offset: u32,
    pub lemma_ids: Vec<u32>,
    pub root_ids: Vec<u32>,     // Root IDs for root-based matching (0 = no root)
}

/// Result of Smith-Waterman alignment
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Alignment {
    pub start_a: usize,
    pub end_a: usize,
    pub start_b: usize,
    pub end_b: usize,
    pub aligned_pairs: Vec<(usize, usize)>, // Matched positions (diagonal moves)
    pub lemma_matches: u32,
    pub substitutions: u32,      // Mismatches on diagonal (neither lemma nor root matched)
    pub root_only_matches: u32,  // Positions where root matched but lemma didn't
    pub gaps: u32,               // Insertions/deletions (up/left moves)
    pub score: i32,
    pub match_weight_sum: f32,   // Sum of weighted lemma matches (document-internal IDF)
}

/// A detected reuse instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReuseEdge {
    pub id: u64,

    // Source location
    pub source_book_id: u32,
    pub source_start_page: (u32, u32),
    pub source_start_offset: u32,
    pub source_end_page: (u32, u32),
    pub source_end_offset: u32,
    pub source_global_start: usize,
    pub source_global_end: usize,

    // Target location
    pub target_book_id: u32,
    pub target_start_page: (u32, u32),
    pub target_start_offset: u32,
    pub target_end_page: (u32, u32),
    pub target_end_offset: u32,
    pub target_global_start: usize,
    pub target_global_end: usize,

    // Raw counts
    pub aligned_length: u32,     // Total alignment operations (diagonal + gaps)
    pub lemma_matches: u32,
    pub substitutions: u32,      // Mismatches on diagonal
    pub root_only_matches: u32,  // Positions where root matched but lemma didn't
    pub gaps: u32,

    // Three orthogonal metrics
    pub core_similarity: f32,    // matches / (matches + subs) - quotation exactness
    pub span_coverage: f32,      // (matches + subs) / aligned_length - reuse vs padding
    pub content_weight: f32,     // match_weight_sum / matches - avg IDF of matches

    // Legacy metrics (kept for backward compatibility)
    pub lemma_similarity: f32,   // lemma_matches / aligned_length
    pub combined_similarity: f32, // (lemma_matches + 0.5 * root_only_matches) / aligned_length
    pub weighted_similarity: f32, // match_weight_sum / aligned_length (IDF-weighted)
    pub avg_match_weight: f32,    // match_weight_sum / lemma_matches (same as content_weight)
}

/// Matching mode for alignment scoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MatchMode {
    /// Only count lemma matches (current/default behavior)
    #[default]
    Lemma,
    /// Only count root matches (ignoring lemma)
    Root,
    /// Lemma match = full score, root-only match = partial score
    Combined,
}

/// Comparison parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonParams {
    pub window_size: usize,
    pub stride: usize,
    pub ngram_size: usize,
    pub min_shared_shingles: usize,
    pub min_length: usize,
    pub min_similarity: f32,
    pub match_score: i32,
    pub mismatch_penalty: i32,
    pub gap_penalty: i32,
    pub brute_force: bool,
    // Root matching parameters
    pub mode: MatchMode,
    pub lemma_score: i32,      // Score for lemma match (default: 2)
    pub root_score: i32,       // Score for root-only match (default: 1)
    // IDF weighting parameters
    pub use_weights: bool,     // Enable document-internal IDF weighting
    pub min_weighted_similarity: Option<f32>,  // Filter by weighted similarity
    // Three-metric filtering
    pub min_core_similarity: Option<f32>,   // Filter by core similarity (quotation exactness)
    pub min_span_coverage: Option<f32>,     // Filter by span coverage (reuse vs padding)
    pub min_content_weight: Option<f32>,    // Filter by content weight (avg IDF)
}

impl Default for ComparisonParams {
    fn default() -> Self {
        Self {
            window_size: 275,
            stride: 60,
            ngram_size: 5,
            min_shared_shingles: 3,
            min_length: 10,
            min_similarity: 0.4,
            match_score: 2,
            mismatch_penalty: -1,
            gap_penalty: -1,
            brute_force: false,
            mode: MatchMode::Lemma,
            lemma_score: 2,
            root_score: 1,
            use_weights: true,
            min_weighted_similarity: None,
            min_core_similarity: None,
            min_span_coverage: None,
            min_content_weight: None,
        }
    }
}

/// Book metadata from Excel
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookMetadata {
    pub id: u32,
    pub corpus: String,
    pub title: String,
    pub author_id: Option<u32>,
    pub death_ah: Option<u32>,
    pub century_ah: Option<u8>,
    pub genre_id: Option<u32>,
    pub page_count: u32,
    pub token_count: u64,
}

/// Full comparison result
#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub version: String,
    pub parameters: ComparisonParams,
    pub book_a: BookMetadata,
    pub book_b: BookMetadata,
    pub summary: ComparisonSummary,
    pub edges: Vec<ReuseEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub edge_count: usize,
    pub total_aligned_tokens: usize,
    pub book_a_coverage: f32,
    pub book_b_coverage: f32,
    pub avg_similarity: f32,
    pub avg_weighted_similarity: f32,  // Average IDF-weighted similarity
}

/// Page metadata from the pages table
#[derive(Debug, Clone, Serialize)]
pub struct PageInfo {
    pub book_id: u32,
    pub part_index: u32,
    pub page_id: u32,
    pub part_label: Option<String>,
    pub page_number: Option<String>,
    pub token_count: u32,
}

/// Corpus statistics
#[derive(Debug, Serialize)]
pub struct CorpusStats {
    pub total_books: u64,
    pub total_pages: u64,
    pub total_tokens: u64,
    pub unique_lemmas: u64,
    pub unique_roots: u64,
    pub token_definitions: u64,
}

/// Book information including token counts
#[derive(Debug, Serialize)]
pub struct BookInfo {
    pub book_id: u32,
    pub page_count: u64,
    pub total_tokens: u64,
    pub unique_lemmas: u64,
    pub pages: Vec<PageInfo>,
}

// ============================================================================
// Enhanced types for text reconstruction and viewer output
// ============================================================================

/// Reference to a passage location with text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassageRef {
    pub book_id: u32,
    pub location: String,                 // "part:start_page.offset → part:end_page.offset"
    pub global_range: (usize, usize),     // (start, end) in flat token array
    pub text: PassageText,
}

/// Alignment information for viewer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentInfo {
    pub length: u32,
    pub lemma_matches: u32,
    pub substitutions: u32,        // Mismatches on diagonal
    pub root_only_matches: u32,
    pub gaps: u32,

    // Three orthogonal metrics
    pub core_similarity: f32,      // matches / (matches + subs) - quotation exactness
    pub span_coverage: f32,        // (matches + subs) / length - reuse vs padding
    pub content_weight: f32,       // avg IDF of matches

    // Legacy metrics (kept for backward compatibility)
    pub similarity: f32,           // lemma_similarity
    pub combined_similarity: f32,  // (lemma + 0.5*root_only) / length
    pub weighted_similarity: f32,  // IDF-weighted similarity
    pub avg_match_weight: f32,     // Same as content_weight
}

/// A reuse edge with reconstructed text for the viewer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReuseEdgeWithText {
    pub id: u64,
    pub source: PassageRef,
    pub target: PassageRef,
    pub alignment: AlignmentInfo,
}

impl ReuseEdgeWithText {
    /// Create from a ReuseEdge by adding text reconstruction
    pub fn from_edge(
        edge: &ReuseEdge,
        source_stream: &BookTokenStream,
        target_stream: &BookTokenStream,
        token_to_surface: &[String],
        context_tokens: usize,
    ) -> Self {
        let source_text = source_stream.get_surface_text_with_context(
            edge.source_global_start,
            edge.source_global_end,
            context_tokens,
            token_to_surface,
        );

        let target_text = target_stream.get_surface_text_with_context(
            edge.target_global_start,
            edge.target_global_end,
            context_tokens,
            token_to_surface,
        );

        let format_location = |start_page: (u32, u32), start_offset: u32, end_page: (u32, u32), end_offset: u32| {
            format!(
                "{}:{}.{} → {}:{}.{}",
                start_page.0, start_page.1, start_offset,
                end_page.0, end_page.1, end_offset
            )
        };

        ReuseEdgeWithText {
            id: edge.id,
            source: PassageRef {
                book_id: edge.source_book_id,
                location: format_location(
                    edge.source_start_page,
                    edge.source_start_offset,
                    edge.source_end_page,
                    edge.source_end_offset,
                ),
                global_range: (edge.source_global_start, edge.source_global_end),
                text: source_text,
            },
            target: PassageRef {
                book_id: edge.target_book_id,
                location: format_location(
                    edge.target_start_page,
                    edge.target_start_offset,
                    edge.target_end_page,
                    edge.target_end_offset,
                ),
                global_range: (edge.target_global_start, edge.target_global_end),
                text: target_text,
            },
            alignment: AlignmentInfo {
                length: edge.aligned_length,
                lemma_matches: edge.lemma_matches,
                substitutions: edge.substitutions,
                root_only_matches: edge.root_only_matches,
                gaps: edge.gaps,
                core_similarity: edge.core_similarity,
                span_coverage: edge.span_coverage,
                content_weight: edge.content_weight,
                similarity: edge.lemma_similarity,
                combined_similarity: edge.combined_similarity,
                weighted_similarity: edge.weighted_similarity,
                avg_match_weight: edge.avg_match_weight,
            },
        }
    }
}

/// Simplified book info for viewer output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerBookInfo {
    pub id: u32,
    pub title: String,
    pub author: String,
    pub death_ah: Option<u32>,
    pub token_count: u64,
    pub page_count: u32,
}

impl From<&BookMetadata> for ViewerBookInfo {
    fn from(meta: &BookMetadata) -> Self {
        ViewerBookInfo {
            id: meta.id,
            title: meta.title.clone(),
            author: String::new(), // Will be filled from external source if available
            death_ah: meta.death_ah,
            token_count: meta.token_count,
            page_count: meta.page_count,
        }
    }
}

/// Full comparison result with text for the viewer
#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResultWithText {
    pub version: String,
    pub generated_at: String,
    pub parameters: ComparisonParams,
    pub book_a: ViewerBookInfo,
    pub book_b: ViewerBookInfo,
    pub summary: ComparisonSummary,
    pub edges: Vec<ReuseEdgeWithText>,
}

