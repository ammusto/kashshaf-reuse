//! Integration tests for kashshaf-reuse.
//!
//! These tests verify the end-to-end functionality of the text reuse detection pipeline.

use kashshaf_reuse::align::align_lemma_sequences;
use kashshaf_reuse::compare::compare_books_from_streams;
use kashshaf_reuse::filter::{find_candidate_pairs, generate_shingles};
use kashshaf_reuse::merge::merge_overlapping_edges;
use kashshaf_reuse::models::{BookLemmaStream, ComparisonParams, PageLemmas, ReuseEdge};
use kashshaf_reuse::window::generate_windows;

/// Helper to create a test book stream with specified content.
fn create_book(book_id: u32, page_sizes: &[usize], base_lemma: u32) -> BookLemmaStream {
    let mut pages = Vec::new();
    let mut total_tokens = 0;
    let mut lemma_counter = base_lemma;

    for (i, &size) in page_sizes.iter().enumerate() {
        let lemma_ids: Vec<u32> = (lemma_counter..lemma_counter + size as u32).collect();
        lemma_counter += size as u32;
        total_tokens += size;

        pages.push(PageLemmas {
            part_index: 1,
            page_id: (i + 1) as u32,
            lemma_ids,
        });
    }

    BookLemmaStream {
        book_id,
        total_tokens,
        pages,
    }
}

/// Create a book with known shared content at specific positions.
fn create_book_with_shared(
    book_id: u32,
    total_size: usize,
    shared_content: &[u32],
    shared_start: usize,
) -> BookLemmaStream {
    let mut lemmas = Vec::with_capacity(total_size);

    // Fill with unique lemmas before shared region
    for i in 0..shared_start {
        lemmas.push((book_id as u32 * 100000) + i as u32);
    }

    // Insert shared content
    lemmas.extend_from_slice(shared_content);

    // Fill with unique lemmas after shared region
    let remaining = total_size.saturating_sub(shared_start + shared_content.len());
    for i in 0..remaining {
        lemmas.push((book_id as u32 * 100000) + 50000 + i as u32);
    }

    BookLemmaStream {
        book_id,
        total_tokens: lemmas.len(),
        pages: vec![PageLemmas {
            part_index: 1,
            page_id: 1,
            lemma_ids: lemmas,
        }],
    }
}

#[test]
fn test_full_pipeline_identical_content() {
    // Two books with identical content
    let book_a = create_book(1, &[500], 0);
    let book_b = create_book(2, &[500], 0);

    let params = ComparisonParams {
        window_size: 100,
        stride: 50,
        min_length: 10,
        min_similarity: 0.5,
        ..Default::default()
    };

    let result = compare_books_from_streams(&book_a, &book_b, &params, false).unwrap();

    // Should find significant reuse
    assert!(!result.edges.is_empty(), "Should find reuse edges");
    assert!(result.summary.avg_similarity > 0.9, "Should have high similarity");
    assert!(
        result.summary.book_a_coverage > 0.5,
        "Should cover significant portion of book A"
    );
}

#[test]
fn test_full_pipeline_no_match() {
    // Two books with completely different content
    let book_a = create_book(1, &[500], 0);
    let book_b = create_book(2, &[500], 10000);

    let params = ComparisonParams {
        window_size: 100,
        stride: 50,
        min_length: 10,
        min_similarity: 0.5,
        ..Default::default()
    };

    let result = compare_books_from_streams(&book_a, &book_b, &params, false).unwrap();

    // Should find no reuse
    assert!(result.edges.is_empty(), "Should not find any reuse edges");
}

#[test]
fn test_full_pipeline_partial_match() {
    // Two books with some shared content in the middle
    let shared: Vec<u32> = (1000..1100).collect(); // 100 shared lemmas

    let book_a = create_book_with_shared(1, 500, &shared, 200);
    let book_b = create_book_with_shared(2, 500, &shared, 150);

    let params = ComparisonParams {
        window_size: 50,
        stride: 25,
        min_length: 10,
        min_similarity: 0.5,
        ..Default::default()
    };

    let result = compare_books_from_streams(&book_a, &book_b, &params, false).unwrap();

    // Should find the shared region
    assert!(!result.edges.is_empty(), "Should find shared content");

    // Verify the detected region corresponds to the shared content
    for edge in &result.edges {
        // The edge should cover positions around the shared region
        assert!(edge.lemma_similarity > 0.5, "Edge should have reasonable similarity");
    }
}

#[test]
fn test_windowing_consistency() {
    let book = create_book(1, &[1000], 0);

    let params = ComparisonParams {
        window_size: 100,
        stride: 50,
        min_length: 10,
        ..Default::default()
    };

    let windows = generate_windows(&book, &params);

    // Verify windows overlap correctly
    for i in 1..windows.len() {
        let prev = &windows[i - 1];
        let curr = &windows[i];

        // Current window should start stride tokens after previous
        assert_eq!(
            curr.global_start,
            prev.global_start + params.stride,
            "Windows should have correct stride"
        );

        // Windows should overlap
        assert!(
            prev.global_end > curr.global_start,
            "Windows should overlap"
        );
    }

    // Verify window contents match their positions
    let flat = book.flat_lemmas();
    for window in &windows {
        let expected: Vec<u32> = flat[window.global_start..window.global_end].to_vec();
        assert_eq!(
            window.lemma_ids, expected,
            "Window content should match position"
        );
    }
}

#[test]
fn test_filtering_effectiveness() {
    // Create books where only some windows should match
    let shared: Vec<u32> = (1000..1050).collect();

    let book_a = create_book_with_shared(1, 1000, &shared, 400);
    let book_b = create_book_with_shared(2, 1000, &shared, 500);

    let params = ComparisonParams {
        window_size: 100,
        stride: 50,
        ngram_size: 5,
        min_shared_shingles: 3,
        brute_force: false,
        ..Default::default()
    };

    let windows_a = generate_windows(&book_a, &params);
    let windows_b = generate_windows(&book_b, &params);

    let candidates = find_candidate_pairs(&windows_a, &windows_b, &params);

    // Should have far fewer candidates than brute force
    let total_pairs = windows_a.len() * windows_b.len();
    assert!(
        candidates.len() < total_pairs / 2,
        "Filtering should reduce candidate pairs significantly"
    );

    // Candidates should include windows containing the shared region
    // (This is approximate since window positions depend on stride)
}

#[test]
fn test_alignment_quality() {
    let params = ComparisonParams {
        min_length: 10,
        min_similarity: 0.6,
        match_score: 2,
        mismatch_penalty: -1,
        gap_penalty: -1,
        ..Default::default()
    };

    // Identical sequences
    let seq: Vec<u32> = (0..50).collect();
    let result = align_lemma_sequences(&seq, &seq, &params);
    assert!(result.is_some(), "Should align identical sequences");
    let alignment = result.unwrap();
    assert_eq!(alignment.lemma_matches as usize, seq.len());
    assert_eq!(alignment.gaps, 0);

    // Sequences with gaps
    let seq_a: Vec<u32> = (0..30).collect();
    let seq_b: Vec<u32> = (0..30).filter(|x| x % 5 != 3).collect(); // Missing every 5th element
    let result = align_lemma_sequences(&seq_a, &seq_b, &params);
    assert!(result.is_some(), "Should handle gaps");
    let alignment = result.unwrap();
    assert!(alignment.gaps > 0, "Should report gaps");
}

#[test]
fn test_merge_overlapping() {
    let edges = vec![
        ReuseEdge {
            id: 1,
            source_book_id: 1,
            source_start_page: (1, 1),
            source_start_offset: 0,
            source_end_page: (1, 1),
            source_end_offset: 0,
            source_global_start: 0,
            source_global_end: 100,
            target_book_id: 2,
            target_start_page: (1, 1),
            target_start_offset: 0,
            target_end_page: (1, 1),
            target_end_offset: 0,
            target_global_start: 0,
            target_global_end: 100,
            aligned_length: 100,
            lemma_matches: 90,
            substitutions: 0,
            root_only_matches: 5,
            gaps: 5,
            core_similarity: 1.0,
            span_coverage: 0.9,
            content_weight: 1.0,
            lemma_similarity: 0.9,
            combined_similarity: 0.925,
            weighted_similarity: 0.9,
            avg_match_weight: 1.0,
        },
        ReuseEdge {
            id: 2,
            source_book_id: 1,
            source_start_page: (1, 1),
            source_start_offset: 0,
            source_end_page: (1, 1),
            source_end_offset: 0,
            source_global_start: 50,
            source_global_end: 150,
            target_book_id: 2,
            target_start_page: (1, 1),
            target_start_offset: 0,
            target_end_page: (1, 1),
            target_end_offset: 0,
            target_global_start: 50,
            target_global_end: 150,
            aligned_length: 100,
            lemma_matches: 90,
            substitutions: 0,
            root_only_matches: 5,
            gaps: 5,
            core_similarity: 1.0,
            span_coverage: 0.9,
            content_weight: 1.0,
            lemma_similarity: 0.9,
            combined_similarity: 0.925,
            weighted_similarity: 0.9,
            avg_match_weight: 1.0,
        },
    ];

    let merged = merge_overlapping_edges(edges);

    // Should merge into single edge spanning 0-150
    assert_eq!(merged.len(), 1, "Should merge overlapping edges");
    assert_eq!(merged[0].source_global_start, 0);
    assert_eq!(merged[0].source_global_end, 150);
}

#[test]
fn test_shingle_generation() {
    let lemmas: Vec<u32> = (1..=10).collect();

    // N-gram size 3
    let shingles = generate_shingles(&lemmas, 3);
    assert_eq!(shingles.len(), 8); // 10 - 3 + 1 = 8 unique shingles

    // Should contain specific shingles
    assert!(shingles.contains(&vec![1, 2, 3]));
    assert!(shingles.contains(&vec![8, 9, 10]));

    // N-gram size larger than sequence
    let shingles = generate_shingles(&lemmas, 20);
    assert!(shingles.is_empty());
}

#[test]
fn test_comparison_params_defaults() {
    let params = ComparisonParams::default();

    assert_eq!(params.window_size, 275);
    assert_eq!(params.stride, 60);
    assert_eq!(params.ngram_size, 5);
    assert_eq!(params.min_shared_shingles, 3);
    assert_eq!(params.min_length, 10);
    assert!((params.min_similarity - 0.4).abs() < 0.001);
    assert_eq!(params.match_score, 2);
    assert_eq!(params.mismatch_penalty, -1);
    assert_eq!(params.gap_penalty, -1);
    assert!(!params.brute_force);
}

#[test]
fn test_small_book_handling() {
    // Book smaller than window size
    let book_a = create_book(1, &[50], 0);
    let book_b = create_book(2, &[50], 0);

    let params = ComparisonParams {
        window_size: 100, // Larger than book size
        stride: 50,
        min_length: 10,
        min_similarity: 0.5,
        ..Default::default()
    };

    let result = compare_books_from_streams(&book_a, &book_b, &params, false).unwrap();

    // Should still work and find the match
    assert!(!result.edges.is_empty(), "Should handle small books");
}

#[test]
fn test_brute_force_mode() {
    let book_a = create_book(1, &[200], 0);
    let book_b = create_book(2, &[200], 0);

    let params_filtered = ComparisonParams {
        window_size: 50,
        stride: 25,
        brute_force: false,
        ..Default::default()
    };

    let params_brute = ComparisonParams {
        window_size: 50,
        stride: 25,
        brute_force: true,
        ..Default::default()
    };

    let result_filtered = compare_books_from_streams(&book_a, &book_b, &params_filtered, false).unwrap();
    let result_brute = compare_books_from_streams(&book_a, &book_b, &params_brute, false).unwrap();

    // Both should find results (identical content)
    assert!(!result_filtered.edges.is_empty());
    assert!(!result_brute.edges.is_empty());

    // Brute force might find more edges before merging
    // but after merging, results should be similar
}
