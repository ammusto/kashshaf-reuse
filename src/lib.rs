//! Kashshaf Text Reuse Detection Library
//!
//! High-performance text reuse detection for premodern Arabic texts.
//! Compares lemma ID sequences to handle morphological variation automatically.
//!
//! # Example
//!
//! ```no_run
//! use kashshaf_reuse::prelude::*;
//! use std::path::Path;
//!
//! let db_path = Path::new("corpus.db");
//! let params = ComparisonParams::default();
//!
//! // Load token-to-lemma mapping
//! let token_to_lemma = load_token_to_lemma(db_path).unwrap();
//!
//! // Load lemma streams for two books
//! let stream_a = load_book_lemma_stream(db_path, 230, &token_to_lemma).unwrap();
//! let stream_b = load_book_lemma_stream(db_path, 553, &token_to_lemma).unwrap();
//!
//! // Compare the books
//! let result = compare_books_from_streams(&stream_a, &stream_b, &params, false).unwrap();
//!
//! println!("Found {} reuse edges", result.edges.len());
//! ```
//!
//! # Text Reconstruction Example
//!
//! ```no_run
//! use kashshaf_reuse::prelude::*;
//! use std::path::Path;
//!
//! let db_path = Path::new("corpus.db");
//! let params = ComparisonParams::default();
//! let context_tokens = 30;
//!
//! // Compare with text reconstruction
//! let result = compare_books_with_text(230, 553, db_path, &params, context_tokens, true).unwrap();
//!
//! // Each edge now includes the actual Arabic text
//! for edge in &result.edges {
//!     println!("Source: {}", edge.source.text.matched);
//!     println!("Target: {}", edge.target.text.matched);
//! }
//! ```

pub mod align;
pub mod compare;
pub mod db;
pub mod extract;
pub mod filter;
pub mod merge;
pub mod models;
pub mod output;
pub mod window;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::align::{align_lemma_sequences, align_lemma_sequences_banded, align_sequences};
    pub use crate::compare::{
        compare_books, compare_books_from_streams, compare_books_from_token_streams,
        compare_books_with_text,
    };
    pub use crate::db::{
        get_lemma_text, get_lemma_texts, load_all_token_mappings, load_book_info,
        load_book_lemma_stream, load_book_token_stream, load_book_token_stream_with_root,
        load_corpus_stats, load_metadata_from_excel, load_token_mappings, load_token_to_lemma,
        load_token_to_root, load_token_to_surface, DbError,
    };
    pub use crate::extract::{
        calculate_lemma_stats, extract_book_lemmas, extract_books_lemmas, find_position_by_page,
        get_lemma_slice, get_page_lemmas, LemmaStats,
    };
    pub use crate::filter::{find_candidate_pairs, generate_shingles, jaccard_similarity};
    pub use crate::merge::{merge_adjacent_edges, merge_overlapping_edges, remove_subsumed_edges};
    pub use crate::models::{
        Alignment, AlignmentInfo, BookInfo, BookLemmaStream, BookMetadata, BookTokenStream,
        ComparisonParams, ComparisonResult, ComparisonResultWithText, ComparisonSummary,
        CorpusStats, MatchMode, OutputFormat, PageInfo, PageLemmas, PageTokens, PassageRef,
        PassageText, ReuseEdge, ReuseEdgeWithText, ViewerBookInfo, Window,
    };
    pub use crate::output::{
        format_edge, format_edge_with_text, format_page_location, generate_viewer_html,
        print_edges, print_edges_with_text, print_summary, print_summary_with_text, write_csv,
        write_csv_file, write_csv_with_text, write_csv_with_text_file, write_json, write_json_file,
        write_json_with_text, write_json_with_text_file, write_viewer_html_file, OutputError,
    };
    pub use crate::window::{calculate_window_count, generate_windows, generate_windows_with_roots};
}

// Re-export commonly used types at the crate root
pub use models::{
    ComparisonParams, ComparisonResult, ComparisonResultWithText, MatchMode, ReuseEdge,
    ReuseEdgeWithText,
};
