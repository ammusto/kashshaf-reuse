//! Pairwise comparison orchestration.
//!
//! This module coordinates the full comparison pipeline between two books:
//! loading, windowing, filtering, alignment, and merging.

use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::align::{align_sequences, align_sequences_weighted};
use crate::db::{
    load_all_token_mappings, load_book_lemma_stream, load_book_token_stream_with_root,
    load_token_to_lemma, DbError,
};
use crate::filter::find_candidate_pairs;
use crate::merge::merge_overlapping_edges;
use crate::models::*;
use crate::window::{generate_windows, generate_windows_with_roots};

/// Static counter for generating unique edge IDs
static EDGE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Compare two books and find all text reuse.
pub fn compare_books(
    book_a_id: u32,
    book_b_id: u32,
    db_path: &Path,
    params: &ComparisonParams,
    show_progress: bool,
) -> Result<ComparisonResult, DbError> {
    // Load token->lemma mapping
    if show_progress {
        eprintln!("Loading token-to-lemma mapping...");
    }
    let token_to_lemma = load_token_to_lemma(db_path)?;

    // Load lemma streams
    if show_progress {
        eprintln!("Loading book {} lemma stream...", book_a_id);
    }
    let stream_a = load_book_lemma_stream(db_path, book_a_id, &token_to_lemma)?;

    if show_progress {
        eprintln!("Loading book {} lemma stream...", book_b_id);
    }
    let stream_b = load_book_lemma_stream(db_path, book_b_id, &token_to_lemma)?;

    compare_books_from_streams(&stream_a, &stream_b, params, show_progress)
}

/// Compare two books given their already-loaded lemma streams.
/// Note: This function uses lemma-only matching for backward compatibility.
/// For root-based matching, use compare_books_from_token_streams.
pub fn compare_books_from_streams(
    stream_a: &BookLemmaStream,
    stream_b: &BookLemmaStream,
    params: &ComparisonParams,
    show_progress: bool,
) -> Result<ComparisonResult, DbError> {
    // Generate windows
    if show_progress {
        eprintln!("Generating windows...");
    }
    let windows_a = generate_windows(stream_a, params);
    let windows_b = generate_windows(stream_b, params);

    if show_progress {
        eprintln!("  Book A: {} windows ({} tokens)", windows_a.len(), stream_a.total_tokens);
        eprintln!("  Book B: {} windows ({} tokens)", windows_b.len(), stream_b.total_tokens);
    }

    // Find candidate pairs
    if show_progress {
        if params.brute_force {
            eprintln!(
                "Mode: BRUTE FORCE (all {} pairs)",
                windows_a.len() * windows_b.len()
            );
        } else {
            eprintln!("Finding candidate pairs (n-gram filtering)...");
        }
    }
    let candidates = find_candidate_pairs(&windows_a, &windows_b, params);

    if show_progress {
        let total_pairs = windows_a.len() * windows_b.len();
        let filter_rate = if total_pairs > 0 {
            100.0 * (1.0 - candidates.len() as f64 / total_pairs as f64)
        } else {
            0.0
        };
        eprintln!(
            "  Candidate pairs: {} ({:.1}% filtered)",
            candidates.len(),
            filter_rate
        );
    }

    // Align candidate pairs in parallel
    let progress = if show_progress {
        let pb = ProgressBar::new(candidates.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    let edges: Vec<ReuseEdge> = candidates
        .par_iter()
        .filter_map(|&(idx_a, idx_b)| {
            let window_a = &windows_a[idx_a];
            let window_b = &windows_b[idx_b];

            // Use align_sequences with root support (root_ids are empty for lemma streams)
            let alignment = align_sequences(
                &window_a.lemma_ids,
                &window_b.lemma_ids,
                &window_a.root_ids,
                &window_b.root_ids,
                params,
            )?;

            if let Some(ref pb) = progress {
                pb.inc(1);
            }

            // Convert alignment to edge
            Some(alignment_to_edge(window_a, window_b, &alignment))
        })
        .collect();

    if let Some(pb) = progress {
        pb.finish_with_message("Done");
    }

    // Merge overlapping edges
    if show_progress {
        eprintln!("Merging overlapping edges ({} raw edges)...", edges.len());
    }
    let merged_edges = merge_overlapping_edges(edges);

    if show_progress {
        eprintln!("  Merged edges: {}", merged_edges.len());
    }

    // Apply metric-based filters
    let filtered_edges = filter_edges_by_params(&merged_edges, params);

    if show_progress && filtered_edges.len() != merged_edges.len() {
        eprintln!("  After filtering: {}", filtered_edges.len());
    }

    // Build result
    let summary = ComparisonSummary {
        edge_count: filtered_edges.len(),
        total_aligned_tokens: filtered_edges
            .iter()
            .map(|e| e.aligned_length as usize)
            .sum(),
        book_a_coverage: calculate_coverage(&filtered_edges, stream_a.book_id, stream_a.total_tokens),
        book_b_coverage: calculate_coverage(&filtered_edges, stream_b.book_id, stream_b.total_tokens),
        avg_similarity: if filtered_edges.is_empty() {
            0.0
        } else {
            filtered_edges.iter().map(|e| e.lemma_similarity).sum::<f32>()
                / filtered_edges.len() as f32
        },
        avg_weighted_similarity: if filtered_edges.is_empty() {
            0.0
        } else {
            filtered_edges.iter().map(|e| e.weighted_similarity).sum::<f32>()
                / filtered_edges.len() as f32
        },
    };

    Ok(ComparisonResult {
        version: env!("CARGO_PKG_VERSION").to_string(),
        parameters: params.clone(),
        book_a: BookMetadata {
            id: stream_a.book_id,
            token_count: stream_a.total_tokens as u64,
            page_count: stream_a.page_count() as u32,
            ..Default::default()
        },
        book_b: BookMetadata {
            id: stream_b.book_id,
            token_count: stream_b.total_tokens as u64,
            page_count: stream_b.page_count() as u32,
            ..Default::default()
        },
        summary,
        edges: filtered_edges,
    })
}

/// Filter edges based on the three-metric parameters.
fn filter_edges_by_params(edges: &[ReuseEdge], params: &ComparisonParams) -> Vec<ReuseEdge> {
    edges
        .iter()
        .filter(|edge| {
            // Check weighted similarity filter
            if let Some(min) = params.min_weighted_similarity {
                if edge.weighted_similarity < min {
                    return false;
                }
            }
            // Check core similarity filter
            if let Some(min) = params.min_core_similarity {
                if edge.core_similarity < min {
                    return false;
                }
            }
            // Check span coverage filter
            if let Some(min) = params.min_span_coverage {
                if edge.span_coverage < min {
                    return false;
                }
            }
            // Check content weight filter
            if let Some(min) = params.min_content_weight {
                if edge.content_weight < min {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

/// Convert an alignment result to a ReuseEdge.
fn alignment_to_edge(window_a: &Window, window_b: &Window, alignment: &Alignment) -> ReuseEdge {
    let id = EDGE_COUNTER.fetch_add(1, Ordering::Relaxed);

    // aligned_length includes diagonal moves (aligned_pairs) + gaps
    let aligned_length = alignment.aligned_pairs.len() as u32 + alignment.gaps;
    let aligned_len_f32 = aligned_length as f32;

    // === Three orthogonal metrics ===

    // Core similarity: quotation exactness (ignores gaps)
    // matches / (matches + substitutions) - how exact is the quoted content
    let match_sub_total = alignment.lemma_matches + alignment.substitutions;
    let core_similarity = if match_sub_total > 0 {
        alignment.lemma_matches as f32 / match_sub_total as f32
    } else {
        0.0
    };

    // Span coverage: reuse vs padding ratio
    // (matches + substitutions) / aligned_length - how much is actual content
    let span_coverage = if aligned_length > 0 {
        match_sub_total as f32 / aligned_len_f32
    } else {
        0.0
    };

    // Content weight: average IDF of matched lemmas
    let content_weight = if alignment.lemma_matches > 0 {
        alignment.match_weight_sum / alignment.lemma_matches as f32
    } else {
        0.0
    };

    // === Legacy metrics (for backward compatibility) ===

    let lemma_similarity = if aligned_len_f32 > 0.0 {
        alignment.lemma_matches as f32 / aligned_len_f32
    } else {
        0.0
    };

    let combined_similarity = if aligned_len_f32 > 0.0 {
        (alignment.lemma_matches as f32 + 0.5 * alignment.root_only_matches as f32) / aligned_len_f32
    } else {
        0.0
    };

    let weighted_similarity = if aligned_len_f32 > 0.0 {
        alignment.match_weight_sum / aligned_len_f32
    } else {
        0.0
    };

    ReuseEdge {
        id,
        source_book_id: window_a.book_id,
        source_start_page: window_a.start_page,
        source_start_offset: window_a.start_offset + alignment.start_a as u32,
        source_end_page: window_a.end_page,
        source_end_offset: window_a.start_offset + alignment.end_a as u32,
        source_global_start: window_a.global_start + alignment.start_a,
        source_global_end: window_a.global_start + alignment.end_a,
        target_book_id: window_b.book_id,
        target_start_page: window_b.start_page,
        target_start_offset: window_b.start_offset + alignment.start_b as u32,
        target_end_page: window_b.end_page,
        target_end_offset: window_b.start_offset + alignment.end_b as u32,
        target_global_start: window_b.global_start + alignment.start_b,
        target_global_end: window_b.global_start + alignment.end_b,
        aligned_length,
        lemma_matches: alignment.lemma_matches,
        substitutions: alignment.substitutions,
        root_only_matches: alignment.root_only_matches,
        gaps: alignment.gaps,
        core_similarity,
        span_coverage,
        content_weight,
        lemma_similarity,
        combined_similarity,
        weighted_similarity,
        avg_match_weight: content_weight, // Same as content_weight
    }
}

/// Calculate coverage as the fraction of a book covered by reuse edges.
fn calculate_coverage(edges: &[ReuseEdge], book_id: u32, total_tokens: usize) -> f32 {
    if total_tokens == 0 {
        return 0.0;
    }

    // Calculate unique covered positions (accounting for overlaps)
    let mut covered_ranges: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| {
            if e.source_book_id == book_id {
                Some((e.source_global_start, e.source_global_end))
            } else if e.target_book_id == book_id {
                Some((e.target_global_start, e.target_global_end))
            } else {
                None
            }
        })
        .collect();

    // Sort and merge overlapping ranges
    covered_ranges.sort_by_key(|r| r.0);
    let merged_ranges = merge_ranges(&covered_ranges);

    // Calculate total covered tokens
    let covered: usize = merged_ranges.iter().map(|(s, e)| e - s).sum();

    covered as f32 / total_tokens as f32
}

/// Merge overlapping ranges into non-overlapping ranges.
fn merge_ranges(ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return Vec::new();
    }

    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut current = ranges[0];

    for &(start, end) in &ranges[1..] {
        if start <= current.1 {
            // Overlapping - extend current range
            current.1 = current.1.max(end);
        } else {
            // Non-overlapping - save current and start new
            merged.push(current);
            current = (start, end);
        }
    }
    merged.push(current);

    merged
}

/// Batch comparison of multiple book pairs.
pub fn compare_book_pairs(
    pairs: &[(u32, u32)],
    db_path: &Path,
    params: &ComparisonParams,
    show_progress: bool,
) -> Result<Vec<ComparisonResult>, DbError> {
    if show_progress {
        eprintln!("Loading token-to-lemma mapping...");
    }
    let token_to_lemma = load_token_to_lemma(db_path)?;

    let results: Vec<Result<ComparisonResult, DbError>> = pairs
        .iter()
        .map(|&(book_a_id, book_b_id)| {
            if show_progress {
                eprintln!("\nComparing books {} and {}...", book_a_id, book_b_id);
            }

            let stream_a = load_book_lemma_stream(db_path, book_a_id, &token_to_lemma)?;
            let stream_b = load_book_lemma_stream(db_path, book_b_id, &token_to_lemma)?;

            compare_books_from_streams(&stream_a, &stream_b, params, show_progress)
        })
        .collect();

    // Collect results, propagating first error if any
    results.into_iter().collect()
}

// ============================================================================
// Enhanced comparison with text reconstruction
// ============================================================================

/// Compare two books and produce results with reconstructed Arabic text.
/// This is the main function for generating viewer-compatible output.
/// Supports all matching modes (lemma, root, combined).
pub fn compare_books_with_text(
    book_a_id: u32,
    book_b_id: u32,
    db_path: &Path,
    params: &ComparisonParams,
    context_tokens: usize,
    show_progress: bool,
) -> Result<ComparisonResultWithText, DbError> {
    // Load all mappings in a single pass for efficiency
    if show_progress {
        eprintln!("Loading token mappings (lemma + root + surface)...");
    }
    let (token_to_lemma, token_to_root, token_to_surface) = load_all_token_mappings(db_path)?;

    // Load token streams (includes token_ids, lemma_ids, and root_ids)
    if show_progress {
        eprintln!("Loading book {} token stream...", book_a_id);
    }
    let stream_a = load_book_token_stream_with_root(db_path, book_a_id, &token_to_lemma, &token_to_root)?;

    if show_progress {
        eprintln!("Loading book {} token stream...", book_b_id);
    }
    let stream_b = load_book_token_stream_with_root(db_path, book_b_id, &token_to_lemma, &token_to_root)?;

    // Run comparison with root support
    let result = compare_token_streams_internal(&stream_a, &stream_b, params, show_progress)?;

    // Reconstruct text for each edge
    if show_progress {
        eprintln!("Reconstructing text for {} edges...", result.edges.len());
    }

    let edges_with_text: Vec<ReuseEdgeWithText> = result
        .edges
        .iter()
        .map(|edge| {
            ReuseEdgeWithText::from_edge(
                edge,
                &stream_a,
                &stream_b,
                &token_to_surface,
                context_tokens,
            )
        })
        .collect();

    // Get current timestamp
    let generated_at = chrono_lite_timestamp();

    Ok(ComparisonResultWithText {
        version: result.version,
        generated_at,
        parameters: result.parameters,
        book_a: ViewerBookInfo::from(&result.book_a),
        book_b: ViewerBookInfo::from(&result.book_b),
        summary: result.summary,
        edges: edges_with_text,
    })
}

/// Internal comparison using token streams with full root support.
fn compare_token_streams_internal(
    stream_a: &BookTokenStream,
    stream_b: &BookTokenStream,
    params: &ComparisonParams,
    show_progress: bool,
) -> Result<ComparisonResult, DbError> {
    // Build lemma weights for IDF weighting (if enabled)
    let (weights_a, weights_b) = if params.use_weights {
        if show_progress {
            eprintln!("Building document-internal IDF weights...");
        }
        let lemmas_a = stream_a.flat_lemma_ids();
        let lemmas_b = stream_b.flat_lemma_ids();
        let max_lemma_id = find_max_lemma_id(stream_a, stream_b);
        (build_lemma_weights(&lemmas_a, max_lemma_id), build_lemma_weights(&lemmas_b, max_lemma_id))
    } else {
        (Vec::new(), Vec::new())
    };

    // Generate windows with root support
    if show_progress {
        eprintln!("Generating windows (with root support)...");
    }
    let windows_a = generate_windows_with_roots(stream_a, params);
    let windows_b = generate_windows_with_roots(stream_b, params);

    if show_progress {
        eprintln!("  Book A: {} windows ({} tokens)", windows_a.len(), stream_a.total_tokens);
        eprintln!("  Book B: {} windows ({} tokens)", windows_b.len(), stream_b.total_tokens);
        eprintln!("  Match mode: {:?}", params.mode);
    }

    // Find candidate pairs
    if show_progress {
        if params.brute_force {
            eprintln!(
                "Mode: BRUTE FORCE (all {} pairs)",
                windows_a.len() * windows_b.len()
            );
        } else {
            eprintln!("Finding candidate pairs (n-gram filtering)...");
        }
    }
    let candidates = find_candidate_pairs(&windows_a, &windows_b, params);

    if show_progress {
        let total_pairs = windows_a.len() * windows_b.len();
        let filter_rate = if total_pairs > 0 {
            100.0 * (1.0 - candidates.len() as f64 / total_pairs as f64)
        } else {
            0.0
        };
        eprintln!(
            "  Candidate pairs: {} ({:.1}% filtered)",
            candidates.len(),
            filter_rate
        );
    }

    // Align candidate pairs in parallel
    let progress = if show_progress {
        let pb = ProgressBar::new(candidates.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    // Share weights across threads
    let weights_a_ref = &weights_a;
    let weights_b_ref = &weights_b;
    let use_weights = params.use_weights;

    let edges: Vec<ReuseEdge> = candidates
        .par_iter()
        .filter_map(|&(idx_a, idx_b)| {
            let window_a = &windows_a[idx_a];
            let window_b = &windows_b[idx_b];

            // Use weighted or unweighted alignment based on params
            let alignment = if use_weights && !weights_a_ref.is_empty() {
                align_sequences_weighted(
                    &window_a.lemma_ids,
                    &window_b.lemma_ids,
                    &window_a.root_ids,
                    &window_b.root_ids,
                    weights_a_ref,
                    weights_b_ref,
                    params,
                )?
            } else {
                align_sequences(
                    &window_a.lemma_ids,
                    &window_b.lemma_ids,
                    &window_a.root_ids,
                    &window_b.root_ids,
                    params,
                )?
            };

            if let Some(ref pb) = progress {
                pb.inc(1);
            }

            // Convert alignment to edge
            Some(alignment_to_edge(window_a, window_b, &alignment))
        })
        .collect();

    if let Some(pb) = progress {
        pb.finish_with_message("Done");
    }

    // Merge overlapping edges
    if show_progress {
        eprintln!("Merging overlapping edges ({} raw edges)...", edges.len());
    }
    let merged_edges = merge_overlapping_edges(edges);

    if show_progress {
        eprintln!("  Merged edges: {}", merged_edges.len());
    }

    // Apply metric-based filters
    let filtered_edges = filter_edges_by_params(&merged_edges, params);

    if show_progress && filtered_edges.len() != merged_edges.len() {
        eprintln!("  After filtering: {}", filtered_edges.len());
    }

    // Build result
    let summary = ComparisonSummary {
        edge_count: filtered_edges.len(),
        total_aligned_tokens: filtered_edges
            .iter()
            .map(|e| e.aligned_length as usize)
            .sum(),
        book_a_coverage: calculate_coverage(&filtered_edges, stream_a.book_id, stream_a.total_tokens),
        book_b_coverage: calculate_coverage(&filtered_edges, stream_b.book_id, stream_b.total_tokens),
        avg_similarity: if filtered_edges.is_empty() {
            0.0
        } else {
            filtered_edges.iter().map(|e| e.lemma_similarity).sum::<f32>()
                / filtered_edges.len() as f32
        },
        avg_weighted_similarity: if filtered_edges.is_empty() {
            0.0
        } else {
            filtered_edges.iter().map(|e| e.weighted_similarity).sum::<f32>()
                / filtered_edges.len() as f32
        },
    };

    Ok(ComparisonResult {
        version: env!("CARGO_PKG_VERSION").to_string(),
        parameters: params.clone(),
        book_a: BookMetadata {
            id: stream_a.book_id,
            token_count: stream_a.total_tokens as u64,
            page_count: stream_a.page_count() as u32,
            ..Default::default()
        },
        book_b: BookMetadata {
            id: stream_b.book_id,
            token_count: stream_b.total_tokens as u64,
            page_count: stream_b.page_count() as u32,
            ..Default::default()
        },
        summary,
        edges: filtered_edges,
    })
}

/// Compare two books from pre-loaded token streams with text reconstruction.
/// Supports all matching modes (lemma, root, combined).
pub fn compare_books_from_token_streams(
    stream_a: &BookTokenStream,
    stream_b: &BookTokenStream,
    token_to_surface: &[String],
    params: &ComparisonParams,
    context_tokens: usize,
    show_progress: bool,
) -> Result<ComparisonResultWithText, DbError> {
    // Run comparison with root support
    let result = compare_token_streams_internal(stream_a, stream_b, params, show_progress)?;

    // Reconstruct text for each edge
    if show_progress {
        eprintln!("Reconstructing text for {} edges...", result.edges.len());
    }

    let edges_with_text: Vec<ReuseEdgeWithText> = result
        .edges
        .iter()
        .map(|edge| {
            ReuseEdgeWithText::from_edge(
                edge,
                stream_a,
                stream_b,
                token_to_surface,
                context_tokens,
            )
        })
        .collect();

    // Get current timestamp
    let generated_at = chrono_lite_timestamp();

    Ok(ComparisonResultWithText {
        version: result.version,
        generated_at,
        parameters: result.parameters,
        book_a: ViewerBookInfo::from(&result.book_a),
        book_b: ViewerBookInfo::from(&result.book_b),
        summary: result.summary,
        edges: edges_with_text,
    })
}

/// Simple timestamp function without external chrono dependency
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();

    // Simple ISO 8601-ish format
    // Calculate approximate date/time (not accounting for leap seconds, etc.)
    let days_since_epoch = secs / 86400;
    let secs_today = secs % 86400;

    // Approximate year/month/day calculation
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days in month_days.iter() {
        if remaining_days < *days {
            break;
        }
        remaining_days -= *days;
        month += 1;
    }

    let day = remaining_days + 1;
    let hour = secs_today / 3600;
    let minute = (secs_today % 3600) / 60;
    let second = secs_today % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PageLemmas;

    fn create_test_stream(book_id: u32, lemmas: Vec<u32>) -> BookLemmaStream {
        let total_tokens = lemmas.len();
        BookLemmaStream {
            book_id,
            total_tokens,
            pages: vec![PageLemmas {
                part_index: 1,
                page_id: 1,
                lemma_ids: lemmas,
            }],
        }
    }

    #[test]
    fn test_compare_identical_streams() {
        let lemmas: Vec<u32> = (0..100).collect();
        let stream_a = create_test_stream(1, lemmas.clone());
        let stream_b = create_test_stream(2, lemmas);

        let params = ComparisonParams {
            window_size: 50,
            stride: 25,
            min_length: 10,
            min_similarity: 0.5,
            ..Default::default()
        };

        let result = compare_books_from_streams(&stream_a, &stream_b, &params, false).unwrap();

        assert!(!result.edges.is_empty());
        assert!(result.summary.avg_similarity > 0.9);
    }

    #[test]
    fn test_compare_no_match() {
        let stream_a = create_test_stream(1, (0..100).collect());
        let stream_b = create_test_stream(2, (1000..1100).collect());

        let params = ComparisonParams {
            window_size: 50,
            stride: 25,
            min_length: 10,
            min_similarity: 0.5,
            ..Default::default()
        };

        let result = compare_books_from_streams(&stream_a, &stream_b, &params, false).unwrap();

        assert!(result.edges.is_empty());
    }

    #[test]
    fn test_coverage_calculation() {
        let edges = vec![
            ReuseEdge {
                id: 1,
                source_book_id: 1,
                source_global_start: 0,
                source_global_end: 50,
                target_book_id: 2,
                target_global_start: 0,
                target_global_end: 50,
                ..Default::default()
            },
            ReuseEdge {
                id: 2,
                source_book_id: 1,
                source_global_start: 25,
                source_global_end: 75,
                target_book_id: 2,
                target_global_start: 25,
                target_global_end: 75,
                ..Default::default()
            },
        ];

        // Total book size is 100, edges cover 0-75 = 75 tokens
        let coverage = calculate_coverage(&edges, 1, 100);
        assert!((coverage - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_merge_ranges() {
        let ranges = vec![(0, 50), (25, 75), (100, 150)];
        let merged = merge_ranges(&ranges);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], (0, 75));
        assert_eq!(merged[1], (100, 150));
    }

    #[test]
    fn test_merge_ranges_empty() {
        let ranges: Vec<(usize, usize)> = vec![];
        let merged = merge_ranges(&ranges);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_merge_ranges_single() {
        let ranges = vec![(0, 50)];
        let merged = merge_ranges(&ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], (0, 50));
    }
}

// Default implementation for ReuseEdge (for tests)
impl Default for ReuseEdge {
    fn default() -> Self {
        ReuseEdge {
            id: 0,
            source_book_id: 0,
            source_start_page: (0, 0),
            source_start_offset: 0,
            source_end_page: (0, 0),
            source_end_offset: 0,
            source_global_start: 0,
            source_global_end: 0,
            target_book_id: 0,
            target_start_page: (0, 0),
            target_start_offset: 0,
            target_end_page: (0, 0),
            target_end_offset: 0,
            target_global_start: 0,
            target_global_end: 0,
            aligned_length: 0,
            lemma_matches: 0,
            substitutions: 0,
            root_only_matches: 0,
            gaps: 0,
            core_similarity: 0.0,
            span_coverage: 0.0,
            content_weight: 0.0,
            lemma_similarity: 0.0,
            combined_similarity: 0.0,
            weighted_similarity: 0.0,
            avg_match_weight: 0.0,
        }
    }
}

// ============================================================================
// Document-internal IDF weighting
// ============================================================================

/// Build document-internal IDF weights for a book's lemma stream.
///
/// For each lemma ℓ in book B:
///   weight_B(ℓ) = ln(total_tokens_B / df_B(ℓ))
///
/// Weights are clamped to [0.5, 3.0] for stability.
///
/// Returns a Vec indexed by lemma_id, with weights for each lemma seen in the book.
pub fn build_lemma_weights(lemma_ids: &[u32], max_lemma_id: usize) -> Vec<f32> {
    // Count document frequency for each lemma
    let mut counts = vec![0u32; max_lemma_id + 1];

    for &id in lemma_ids {
        if (id as usize) < counts.len() {
            counts[id as usize] += 1;
        }
    }

    let total = lemma_ids.len() as f32;
    let mut weights = vec![0.0f32; max_lemma_id + 1];

    for (id, &df) in counts.iter().enumerate() {
        if df > 0 {
            // IDF formula: ln(total / df), clamped to [0.5, 3.0]
            let w = (total / df as f32).ln().clamp(0.5, 3.0);
            weights[id] = w;
        }
    }

    weights
}

/// Find the maximum lemma ID in the token streams.
pub fn find_max_lemma_id(stream_a: &BookTokenStream, stream_b: &BookTokenStream) -> usize {
    let max_a = stream_a.flat_lemma_ids().iter().copied().max().unwrap_or(0) as usize;
    let max_b = stream_b.flat_lemma_ids().iter().copied().max().unwrap_or(0) as usize;
    max_a.max(max_b)
}
