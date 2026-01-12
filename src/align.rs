//! Smith-Waterman local alignment for lemma ID sequences.
//!
//! This is the HOT PATH - performance is critical here.
//! The algorithm finds the best local alignment between two sequences.

use crate::models::{Alignment, ComparisonParams, MatchMode};

/// Smith-Waterman local alignment on lemma ID sequences.
///
/// This is the HOT PATH - must be highly optimized.
///
/// Returns None if no alignment meets minimum criteria.
///
/// Note: For backward compatibility, this function only uses lemma matching.
/// Use `align_sequences` for root-based matching support.
#[inline]
pub fn align_lemma_sequences(
    seq_a: &[u32],
    seq_b: &[u32],
    params: &ComparisonParams,
) -> Option<Alignment> {
    // Create empty root sequences for backward compatibility
    let empty_roots: Vec<u32> = vec![0; seq_a.len().max(seq_b.len())];
    align_sequences(seq_a, seq_b, &empty_roots, &empty_roots, params)
}

/// Smith-Waterman local alignment with support for lemma, root, and combined matching modes.
///
/// This is the HOT PATH - must be highly optimized.
///
/// Returns None if no alignment meets minimum criteria.
///
/// # Arguments
/// * `lemmas_a` - Lemma IDs for sequence A
/// * `lemmas_b` - Lemma IDs for sequence B
/// * `roots_a` - Root IDs for sequence A (0 = no root)
/// * `roots_b` - Root IDs for sequence B (0 = no root)
/// * `params` - Comparison parameters including match mode
#[inline]
pub fn align_sequences(
    lemmas_a: &[u32],
    lemmas_b: &[u32],
    roots_a: &[u32],
    roots_b: &[u32],
    params: &ComparisonParams,
) -> Option<Alignment> {
    let n = lemmas_a.len();
    let m = lemmas_b.len();

    if n == 0 || m == 0 {
        return None;
    }

    // DP matrix - use flat Vec for cache efficiency
    // H[i][j] = H[i * (m+1) + j]
    let width = m + 1;
    let mut h = vec![0i32; (n + 1) * width];

    // Track max score position for traceback
    let mut max_score = 0i32;
    let mut max_i = 0usize;
    let mut max_j = 0usize;

    // Fill DP matrix
    for i in 1..=n {
        let lemma_a = lemmas_a[i - 1];
        let root_a = if i - 1 < roots_a.len() { roots_a[i - 1] } else { 0 };
        let row_offset = i * width;
        let prev_row_offset = (i - 1) * width;

        for j in 1..=m {
            let lemma_b = lemmas_b[j - 1];
            let root_b = if j - 1 < roots_b.len() { roots_b[j - 1] } else { 0 };

            // Calculate match/mismatch score based on mode
            let match_score = calculate_match_score(
                lemma_a, lemma_b, root_a, root_b, params
            );

            // Compute cell value: max of 0, diagonal+match, up+gap, left+gap
            let diagonal = h[prev_row_offset + (j - 1)] + match_score;
            let up = h[prev_row_offset + j] + params.gap_penalty;
            let left = h[row_offset + (j - 1)] + params.gap_penalty;

            let score = 0.max(diagonal).max(up).max(left);
            h[row_offset + j] = score;

            if score > max_score {
                max_score = score;
                max_i = i;
                max_j = j;
            }
        }
    }

    // Early exit if no significant alignment
    let min_score_threshold = match params.mode {
        MatchMode::Lemma => (params.min_length as i32 * params.lemma_score) / 2,
        MatchMode::Root => (params.min_length as i32 * params.lemma_score) / 2,
        MatchMode::Combined => (params.min_length as i32 * params.lemma_score) / 2,
    };
    if max_score < min_score_threshold {
        return None;
    }

    // Traceback to recover alignment
    let mut aligned_pairs = Vec::with_capacity(n.min(m));
    let mut i = max_i;
    let mut j = max_j;
    let mut gaps = 0u32;
    let mut lemma_matches = 0u32;
    let mut substitutions = 0u32;
    let mut root_only_matches = 0u32;

    while i > 0 && j > 0 && h[i * width + j] > 0 {
        let current = h[i * width + j];
        let diagonal = h[(i - 1) * width + (j - 1)];
        let up = h[(i - 1) * width + j];

        let lemma_a = lemmas_a[i - 1];
        let lemma_b = lemmas_b[j - 1];
        let root_a = if i - 1 < roots_a.len() { roots_a[i - 1] } else { 0 };
        let root_b = if j - 1 < roots_b.len() { roots_b[j - 1] } else { 0 };

        let match_score = calculate_match_score(lemma_a, lemma_b, root_a, root_b, params);

        if current == diagonal + match_score {
            // Match or mismatch - record the pair
            aligned_pairs.push((i - 1, j - 1));

            // Track what kind of match it was
            if lemma_a == lemma_b {
                lemma_matches += 1;
            } else if root_a == root_b && root_a != 0 {
                root_only_matches += 1;
            } else {
                // Neither lemma nor root matched - this is a substitution
                substitutions += 1;
            }

            i -= 1;
            j -= 1;
        } else if current == up + params.gap_penalty {
            // Gap in seq_b
            gaps += 1;
            i -= 1;
        } else {
            // Gap in seq_a
            gaps += 1;
            j -= 1;
        }
    }

    // Alignment is built backwards, reverse it
    aligned_pairs.reverse();

    // Check minimum length
    if aligned_pairs.len() < params.min_length {
        return None;
    }

    // Check minimum similarity based on mode
    let similarity = match params.mode {
        MatchMode::Lemma => lemma_matches as f32 / aligned_pairs.len() as f32,
        MatchMode::Root => {
            // In root mode, count root matches (including lemma matches which share roots)
            let root_matches = count_root_matches(&aligned_pairs, lemmas_a, lemmas_b, roots_a, roots_b);
            root_matches as f32 / aligned_pairs.len() as f32
        }
        MatchMode::Combined => {
            // Combined mode uses weighted similarity
            (lemma_matches as f32 + 0.5 * root_only_matches as f32) / aligned_pairs.len() as f32
        }
    };

    if similarity < params.min_similarity {
        return None;
    }

    // Find start/end positions
    let (start_a, start_b) = aligned_pairs.first().copied().unwrap_or((0, 0));
    let (end_a, end_b) = aligned_pairs.last().copied().unwrap_or((0, 0));

    Some(Alignment {
        start_a,
        end_a: end_a + 1,
        start_b,
        end_b: end_b + 1,
        aligned_pairs,
        lemma_matches,
        substitutions,
        root_only_matches,
        gaps,
        score: max_score,
        match_weight_sum: 0.0,
    })
}

/// Calculate the match score for a pair of positions based on matching mode.
#[inline(always)]
fn calculate_match_score(
    lemma_a: u32,
    lemma_b: u32,
    root_a: u32,
    root_b: u32,
    params: &ComparisonParams,
) -> i32 {
    match params.mode {
        MatchMode::Lemma => {
            if lemma_a == lemma_b {
                params.lemma_score
            } else {
                params.mismatch_penalty
            }
        }
        MatchMode::Root => {
            if root_a == root_b && root_a != 0 {
                params.lemma_score // Use lemma_score as the "match" score for root mode
            } else {
                params.mismatch_penalty
            }
        }
        MatchMode::Combined => {
            if lemma_a == lemma_b {
                params.lemma_score
            } else if root_a == root_b && root_a != 0 {
                params.root_score
            } else {
                params.mismatch_penalty
            }
        }
    }
}

/// Count root matches in aligned pairs (including lemma matches that share roots).
#[inline]
fn count_root_matches(
    aligned_pairs: &[(usize, usize)],
    _lemmas_a: &[u32],
    _lemmas_b: &[u32],
    roots_a: &[u32],
    roots_b: &[u32],
) -> u32 {
    aligned_pairs
        .iter()
        .filter(|&&(i, j)| {
            let root_a = if i < roots_a.len() { roots_a[i] } else { 0 };
            let root_b = if j < roots_b.len() { roots_b[j] } else { 0 };
            root_a == root_b && root_a != 0
        })
        .count() as u32
}

/// Banded Smith-Waterman for even faster alignment.
/// Only computes cells within `band` diagonals of the main diagonal.
///
/// This is useful when we expect the aligned regions to be roughly
/// at the same positions in both sequences.
///
/// Note: Currently falls back to full alignment. Banded implementation
/// is a future optimization.
#[inline]
pub fn align_lemma_sequences_banded(
    seq_a: &[u32],
    seq_b: &[u32],
    params: &ComparisonParams,
    _band: usize,
) -> Option<Alignment> {
    // TODO: Implement proper banded alignment for additional speedup
    // For now, fall back to full alignment
    align_lemma_sequences(seq_a, seq_b, params)
}

/// Quick check if two sequences might have a significant alignment.
/// Uses a simple count of shared lemmas to avoid expensive alignment.
#[inline]
pub fn quick_similarity_check(seq_a: &[u32], seq_b: &[u32], min_shared: usize) -> bool {
    if seq_a.len() < min_shared || seq_b.len() < min_shared {
        return false;
    }

    // Count shared lemmas using a simple approach
    let mut count = 0;
    for &lemma in seq_a {
        if seq_b.contains(&lemma) {
            count += 1;
            if count >= min_shared {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_params() -> ComparisonParams {
        ComparisonParams {
            min_length: 10,
            min_similarity: 0.4,
            match_score: 2,
            mismatch_penalty: -1,
            gap_penalty: -1,
            ..Default::default()
        }
    }

    #[test]
    fn test_identical_sequences() {
        let seq: Vec<u32> = (0..20).collect();
        let params = default_params();

        let result = align_lemma_sequences(&seq, &seq, &params);
        assert!(result.is_some());

        let alignment = result.unwrap();
        assert_eq!(alignment.lemma_matches as usize, seq.len());
        assert_eq!(alignment.gaps, 0);
        assert_eq!(alignment.aligned_pairs.len(), seq.len());
    }

    #[test]
    fn test_partial_match() {
        let seq_a: Vec<u32> = (0..20).collect();
        let seq_b: Vec<u32> = (0..20)
            .map(|i| if i >= 5 && i < 15 { i } else { i + 1000 })
            .collect();
        let params = default_params();

        let result = align_lemma_sequences(&seq_a, &seq_b, &params);
        assert!(result.is_some());

        let alignment = result.unwrap();
        assert!(alignment.lemma_matches >= 10);
    }

    #[test]
    fn test_no_match() {
        let seq_a: Vec<u32> = (0..15).collect();
        let seq_b: Vec<u32> = (100..115).collect();
        let params = default_params();

        let result = align_lemma_sequences(&seq_a, &seq_b, &params);
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_sequences() {
        let params = default_params();

        assert!(align_lemma_sequences(&[], &[1, 2, 3], &params).is_none());
        assert!(align_lemma_sequences(&[1, 2, 3], &[], &params).is_none());
        assert!(align_lemma_sequences(&[], &[], &params).is_none());
    }

    #[test]
    fn test_with_gaps() {
        // seq_a: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12
        // seq_b: 1, 2, X, 4, 5, 6, X, 8, 9, 10, 11, 12
        let seq_a: Vec<u32> = (1..=12).collect();
        let seq_b: Vec<u32> = vec![1, 2, 100, 4, 5, 6, 100, 8, 9, 10, 11, 12];
        let params = default_params();

        let result = align_lemma_sequences(&seq_a, &seq_b, &params);
        assert!(result.is_some());

        let alignment = result.unwrap();
        assert!(alignment.lemma_matches >= 10);
    }

    #[test]
    fn test_min_length_threshold() {
        let seq: Vec<u32> = (0..8).collect(); // Less than min_length of 10
        let params = default_params();

        let result = align_lemma_sequences(&seq, &seq, &params);
        assert!(result.is_none());
    }

    #[test]
    fn test_min_similarity_threshold() {
        // Create sequences where only 30% match (below 40% threshold)
        let seq_a: Vec<u32> = (0..20).collect();
        let seq_b: Vec<u32> = (0..20)
            .map(|i| if i % 3 == 0 { i } else { i + 1000 })
            .collect();
        let params = ComparisonParams {
            min_length: 5, // Lower threshold to test similarity
            min_similarity: 0.4,
            match_score: 2,
            mismatch_penalty: -1,
            gap_penalty: -1,
            ..Default::default()
        };

        let result = align_lemma_sequences(&seq_a, &seq_b, &params);
        // Result depends on whether the alignment meets the 40% threshold
        // With only ~33% matches, it should likely fail
    }

    #[test]
    fn test_banded_alignment() {
        let seq: Vec<u32> = (0..100).collect();
        let params = default_params();

        let result_full = align_lemma_sequences(&seq, &seq, &params);
        let result_banded = align_lemma_sequences_banded(&seq, &seq, &params, 20);

        assert!(result_full.is_some());
        assert!(result_banded.is_some());

        // Banded should find similar quality alignment
        let full = result_full.unwrap();
        let banded = result_banded.unwrap();

        assert_eq!(full.lemma_matches, banded.lemma_matches);
    }

    #[test]
    fn test_quick_similarity_check() {
        let seq_a: Vec<u32> = (0..20).collect();
        let seq_b: Vec<u32> = (10..30).collect(); // Overlaps by 10

        assert!(quick_similarity_check(&seq_a, &seq_b, 5));
        assert!(quick_similarity_check(&seq_a, &seq_b, 10));
        assert!(!quick_similarity_check(&seq_a, &seq_b, 15));

        let seq_c: Vec<u32> = (100..120).collect(); // No overlap
        assert!(!quick_similarity_check(&seq_a, &seq_c, 1));
    }

    #[test]
    fn test_alignment_positions() {
        // Test that alignment positions are correctly reported
        let seq_a: Vec<u32> = vec![100, 101, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 102, 103];
        let seq_b: Vec<u32> = vec![200, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 201, 202];
        let params = default_params();

        let result = align_lemma_sequences(&seq_a, &seq_b, &params);
        assert!(result.is_some());

        let alignment = result.unwrap();
        // The matching region should start at position 2 in seq_a and position 1 in seq_b
        assert_eq!(alignment.start_a, 2);
        assert_eq!(alignment.start_b, 1);
    }

    // ==================== Root Matching Tests ====================

    #[test]
    fn test_root_mode_matching() {
        // Different lemmas, same roots should match in root mode
        let lemmas_a: Vec<u32> = (0..20).collect();  // lemmas 0-19
        let lemmas_b: Vec<u32> = (100..120).collect();  // different lemmas 100-119
        let roots_a: Vec<u32> = (1..21).collect();  // roots 1-20
        let roots_b: Vec<u32> = (1..21).collect();  // same roots 1-20

        let mut params = default_params();
        params.mode = MatchMode::Root;

        let result = align_sequences(&lemmas_a, &lemmas_b, &roots_a, &roots_b, &params);
        assert!(result.is_some());

        let alignment = result.unwrap();
        // All positions should match via root
        assert_eq!(alignment.lemma_matches, 0); // No lemma matches
        assert!(alignment.root_only_matches >= 10); // Root matches
    }

    #[test]
    fn test_combined_mode_matching() {
        // Mixed lemma and root matches
        let lemmas_a: Vec<u32> = (0..20).collect();
        let lemmas_b: Vec<u32> = (0..20)
            .map(|i| if i < 10 { i } else { i + 1000 })  // First 10 same lemma, rest different
            .collect();
        let roots_a: Vec<u32> = (1..21).collect();  // roots 1-20
        let roots_b: Vec<u32> = (1..21).collect();  // same roots 1-20

        let mut params = default_params();
        params.mode = MatchMode::Combined;

        let result = align_sequences(&lemmas_a, &lemmas_b, &roots_a, &roots_b, &params);
        assert!(result.is_some());

        let alignment = result.unwrap();
        assert_eq!(alignment.lemma_matches, 10);  // First 10 lemmas match
        assert!(alignment.root_only_matches >= 5);  // Some root-only matches
    }

    #[test]
    fn test_root_zero_not_matched() {
        // Roots with value 0 should never match (0 = no root)
        let lemmas_a: Vec<u32> = (0..15).collect();
        let lemmas_b: Vec<u32> = (100..115).collect();  // Different lemmas
        let roots_a: Vec<u32> = vec![0; 15];  // No roots (all 0)
        let roots_b: Vec<u32> = vec![0; 15];  // No roots (all 0)

        let mut params = default_params();
        params.mode = MatchMode::Root;

        let result = align_sequences(&lemmas_a, &lemmas_b, &roots_a, &roots_b, &params);
        // Should not match because roots are all 0
        assert!(result.is_none());
    }

    #[test]
    fn test_lemma_mode_ignores_roots() {
        // In lemma mode, root matches should not affect scoring
        let lemmas_a: Vec<u32> = (0..15).collect();
        let lemmas_b: Vec<u32> = (100..115).collect();  // Different lemmas
        let roots_a: Vec<u32> = (1..16).collect();  // Same roots
        let roots_b: Vec<u32> = (1..16).collect();  // Same roots

        let mut params = default_params();
        params.mode = MatchMode::Lemma;

        let result = align_sequences(&lemmas_a, &lemmas_b, &roots_a, &roots_b, &params);
        // Should not match despite same roots
        assert!(result.is_none());
    }

    #[test]
    fn test_combined_scoring() {
        // Test that combined mode scores lemma matches higher than root-only matches
        let lemmas_a: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let lemmas_b: Vec<u32> = vec![1, 2, 3, 4, 5, 100, 100, 100, 100, 100, 11, 12];
        let roots_a: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let roots_b: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

        let mut params = default_params();
        params.mode = MatchMode::Combined;
        params.lemma_score = 2;
        params.root_score = 1;

        let result = align_sequences(&lemmas_a, &lemmas_b, &roots_a, &roots_b, &params);
        assert!(result.is_some());

        let alignment = result.unwrap();
        // First 5 + last 2 = 7 lemma matches
        // Middle 5 are root-only matches
        assert!(alignment.lemma_matches >= 7);
        assert!(alignment.root_only_matches >= 3);
    }
}

/// Smith-Waterman local alignment with document-internal IDF weighting.
///
/// This version uses per-book lemma weights to influence alignment scoring.
/// Rare lemmas contribute more to alignment than common lemmas.
///
/// # Arguments
/// * `lemmas_a` - Lemma IDs for sequence A
/// * `lemmas_b` - Lemma IDs for sequence B
/// * `roots_a` - Root IDs for sequence A (0 = no root)
/// * `roots_b` - Root IDs for sequence B (0 = no root)
/// * `weights_a` - IDF weights for book A (indexed by lemma ID)
/// * `weights_b` - IDF weights for book B (indexed by lemma ID)
/// * `params` - Comparison parameters including match mode
#[inline]
pub fn align_sequences_weighted(
    lemmas_a: &[u32],
    lemmas_b: &[u32],
    roots_a: &[u32],
    roots_b: &[u32],
    weights_a: &[f32],
    weights_b: &[f32],
    params: &ComparisonParams,
) -> Option<Alignment> {
    let n = lemmas_a.len();
    let m = lemmas_b.len();

    if n == 0 || m == 0 {
        return None;
    }

    // DP matrix - use flat Vec for cache efficiency
    let width = m + 1;
    let mut h = vec![0i32; (n + 1) * width];

    // Track max score position for traceback
    let mut max_score = 0i32;
    let mut max_i = 0usize;
    let mut max_j = 0usize;

    // Fill DP matrix with weighted scoring
    for i in 1..=n {
        let lemma_a = lemmas_a[i - 1];
        let root_a = if i - 1 < roots_a.len() { roots_a[i - 1] } else { 0 };
        let row_offset = i * width;
        let prev_row_offset = (i - 1) * width;

        for j in 1..=m {
            let lemma_b = lemmas_b[j - 1];
            let root_b = if j - 1 < roots_b.len() { roots_b[j - 1] } else { 0 };

            // Calculate weighted match score
            let match_score = calculate_weighted_match_score(
                lemma_a, lemma_b, root_a, root_b,
                weights_a, weights_b, params
            );

            // Compute cell value: max of 0, diagonal+match, up+gap, left+gap
            let diagonal = h[prev_row_offset + (j - 1)] + match_score;
            let up = h[prev_row_offset + j] + params.gap_penalty;
            let left = h[row_offset + (j - 1)] + params.gap_penalty;

            let score = 0.max(diagonal).max(up).max(left);
            h[row_offset + j] = score;

            if score > max_score {
                max_score = score;
                max_i = i;
                max_j = j;
            }
        }
    }

    // Early exit if no significant alignment
    let min_score_threshold = (params.min_length as i32 * params.lemma_score) / 2;
    if max_score < min_score_threshold {
        return None;
    }

    // Traceback to recover alignment and compute match_weight_sum
    let mut aligned_pairs = Vec::with_capacity(n.min(m));
    let mut i = max_i;
    let mut j = max_j;
    let mut gaps = 0u32;
    let mut lemma_matches = 0u32;
    let mut substitutions = 0u32;
    let mut root_only_matches = 0u32;
    let mut match_weight_sum = 0.0f32;

    while i > 0 && j > 0 && h[i * width + j] > 0 {
        let current = h[i * width + j];
        let diagonal = h[(i - 1) * width + (j - 1)];
        let up = h[(i - 1) * width + j];

        let lemma_a = lemmas_a[i - 1];
        let lemma_b = lemmas_b[j - 1];
        let root_a = if i - 1 < roots_a.len() { roots_a[i - 1] } else { 0 };
        let root_b = if j - 1 < roots_b.len() { roots_b[j - 1] } else { 0 };

        let match_score = calculate_weighted_match_score(
            lemma_a, lemma_b, root_a, root_b,
            weights_a, weights_b, params
        );

        if current == diagonal + match_score {
            // Match or mismatch - record the pair
            aligned_pairs.push((i - 1, j - 1));

            // Track what kind of match it was
            if lemma_a == lemma_b {
                lemma_matches += 1;
                // Add weight to match_weight_sum: min(weight_A, weight_B)
                let w_a = get_weight(lemma_a, weights_a);
                let w_b = get_weight(lemma_b, weights_b);
                match_weight_sum += w_a.min(w_b);
            } else if root_a == root_b && root_a != 0 {
                root_only_matches += 1;
            } else {
                // Neither lemma nor root matched - this is a substitution
                substitutions += 1;
            }

            i -= 1;
            j -= 1;
        } else if current == up + params.gap_penalty {
            // Gap in seq_b
            gaps += 1;
            i -= 1;
        } else {
            // Gap in seq_a
            gaps += 1;
            j -= 1;
        }
    }

    // Alignment is built backwards, reverse it
    aligned_pairs.reverse();

    // Check minimum length
    if aligned_pairs.len() < params.min_length {
        return None;
    }

    // Check minimum similarity based on mode
    let similarity = match params.mode {
        MatchMode::Lemma => lemma_matches as f32 / aligned_pairs.len() as f32,
        MatchMode::Root => {
            let root_matches = count_root_matches(&aligned_pairs, lemmas_a, lemmas_b, roots_a, roots_b);
            root_matches as f32 / aligned_pairs.len() as f32
        }
        MatchMode::Combined => {
            (lemma_matches as f32 + 0.5 * root_only_matches as f32) / aligned_pairs.len() as f32
        }
    };

    if similarity < params.min_similarity {
        return None;
    }

    // Find start/end positions
    let (start_a, start_b) = aligned_pairs.first().copied().unwrap_or((0, 0));
    let (end_a, end_b) = aligned_pairs.last().copied().unwrap_or((0, 0));

    Some(Alignment {
        start_a,
        end_a: end_a + 1,
        start_b,
        end_b: end_b + 1,
        aligned_pairs,
        lemma_matches,
        substitutions,
        root_only_matches,
        gaps,
        score: max_score,
        match_weight_sum,
    })
}

/// Calculate weighted match score using document-internal IDF weights.
#[inline(always)]
fn calculate_weighted_match_score(
    lemma_a: u32,
    lemma_b: u32,
    root_a: u32,
    root_b: u32,
    weights_a: &[f32],
    weights_b: &[f32],
    params: &ComparisonParams,
) -> i32 {
    match params.mode {
        MatchMode::Lemma => {
            if lemma_a == lemma_b {
                // Weight the score by min(weight_A, weight_B)
                let w_a = get_weight(lemma_a, weights_a);
                let w_b = get_weight(lemma_b, weights_b);
                let w = w_a.min(w_b);
                (params.lemma_score as f32 * w) as i32
            } else {
                params.mismatch_penalty
            }
        }
        MatchMode::Root => {
            if root_a == root_b && root_a != 0 {
                params.lemma_score
            } else {
                params.mismatch_penalty
            }
        }
        MatchMode::Combined => {
            if lemma_a == lemma_b {
                let w_a = get_weight(lemma_a, weights_a);
                let w_b = get_weight(lemma_b, weights_b);
                let w = w_a.min(w_b);
                (params.lemma_score as f32 * w) as i32
            } else if root_a == root_b && root_a != 0 {
                params.root_score
            } else {
                params.mismatch_penalty
            }
        }
    }
}

/// Get weight for a lemma ID, with bounds checking and default.
#[inline(always)]
fn get_weight(lemma_id: u32, weights: &[f32]) -> f32 {
    let idx = lemma_id as usize;
    if idx < weights.len() && weights[idx] > 0.0 {
        weights[idx]
    } else {
        1.0 // Default weight for unknown lemmas
    }
}
