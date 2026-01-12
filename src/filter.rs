//! N-gram shingling and candidate pair filtering.
//!
//! This module implements efficient filtering to reduce the number of
//! window pairs that need full Smith-Waterman alignment.

use crate::models::{ComparisonParams, Window};
use std::collections::{HashMap, HashSet};

/// Generate n-gram shingles from a lemma sequence.
///
/// A shingle is a contiguous sequence of n lemma IDs.
/// Returns a set of unique shingles found in the sequence.
pub fn generate_shingles(lemma_ids: &[u32], n: usize) -> HashSet<Vec<u32>> {
    if lemma_ids.len() < n || n == 0 {
        return HashSet::new();
    }

    lemma_ids.windows(n).map(|w| w.to_vec()).collect()
}

/// Find candidate window pairs that share enough shingles.
///
/// This function builds an inverted index of shingles from windows_b,
/// then queries it with shingles from windows_a to find potential matches.
///
/// Returns pairs of window indices (idx_a, idx_b) that should be aligned.
pub fn find_candidate_pairs(
    windows_a: &[Window],
    windows_b: &[Window],
    params: &ComparisonParams,
) -> Vec<(usize, usize)> {
    if params.brute_force {
        // Return all pairs for brute force mode
        return generate_all_pairs(windows_a.len(), windows_b.len());
    }

    // Build shingle index for windows_b
    // Map: shingle -> list of window indices containing it
    let shingle_index = build_shingle_index(windows_b, params.ngram_size);

    // For each window in A, find windows in B that share enough shingles
    let mut candidates = Vec::new();

    for (idx_a, window_a) in windows_a.iter().enumerate() {
        let shingles_a = generate_shingles(&window_a.lemma_ids, params.ngram_size);

        // Count shared shingles with each window in B
        let mut shared_counts: HashMap<usize, usize> = HashMap::new();

        for shingle in &shingles_a {
            if let Some(matching_windows) = shingle_index.get(shingle) {
                for &idx_b in matching_windows {
                    *shared_counts.entry(idx_b).or_default() += 1;
                }
            }
        }

        // Keep pairs that meet threshold
        for (idx_b, count) in shared_counts {
            if count >= params.min_shared_shingles {
                candidates.push((idx_a, idx_b));
            }
        }
    }

    candidates
}

/// Build an inverted index mapping shingles to window indices
fn build_shingle_index(windows: &[Window], ngram_size: usize) -> HashMap<Vec<u32>, Vec<usize>> {
    let mut index: HashMap<Vec<u32>, Vec<usize>> = HashMap::new();

    for (idx, window) in windows.iter().enumerate() {
        let shingles = generate_shingles(&window.lemma_ids, ngram_size);
        for shingle in shingles {
            index.entry(shingle).or_default().push(idx);
        }
    }

    index
}

/// Generate all pairs (brute force mode)
fn generate_all_pairs(len_a: usize, len_b: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::with_capacity(len_a * len_b);
    for i in 0..len_a {
        for j in 0..len_b {
            pairs.push((i, j));
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_window(book_id: u32, idx: u32, lemmas: Vec<u32>) -> Window {
        let len = lemmas.len();
        Window {
            book_id,
            window_idx: idx,
            global_start: 0,
            global_end: len,
            start_page: (1, 1),
            start_offset: 0,
            end_page: (1, 1),
            end_offset: 0,
            lemma_ids: lemmas,
            root_ids: vec![0; len],  // Empty roots for testing
        }
    }

    #[test]
    fn test_generate_shingles_empty() {
        let shingles = generate_shingles(&[], 3);
        assert!(shingles.is_empty());
    }

    #[test]
    fn test_generate_shingles_too_short() {
        let shingles = generate_shingles(&[1, 2], 3);
        assert!(shingles.is_empty());
    }

    #[test]
    fn test_generate_shingles_exact_size() {
        let shingles = generate_shingles(&[1, 2, 3], 3);
        assert_eq!(shingles.len(), 1);
        assert!(shingles.contains(&vec![1, 2, 3]));
    }

    #[test]
    fn test_generate_shingles_multiple() {
        let shingles = generate_shingles(&[1, 2, 3, 4, 5], 3);
        assert_eq!(shingles.len(), 3);
        assert!(shingles.contains(&vec![1, 2, 3]));
        assert!(shingles.contains(&vec![2, 3, 4]));
        assert!(shingles.contains(&vec![3, 4, 5]));
    }

    #[test]
    fn test_generate_shingles_with_duplicates() {
        // Repeated shingles should result in a smaller set
        let shingles = generate_shingles(&[1, 2, 1, 2, 1, 2], 2);
        assert_eq!(shingles.len(), 2); // [1,2] and [2,1]
    }

    #[test]
    fn test_find_candidate_pairs_brute_force() {
        let windows_a = vec![
            create_test_window(1, 0, vec![1, 2, 3]),
            create_test_window(1, 1, vec![4, 5, 6]),
        ];
        let windows_b = vec![
            create_test_window(2, 0, vec![7, 8, 9]),
            create_test_window(2, 1, vec![10, 11, 12]),
            create_test_window(2, 2, vec![13, 14, 15]),
        ];

        let params = ComparisonParams {
            brute_force: true,
            ..Default::default()
        };

        let pairs = find_candidate_pairs(&windows_a, &windows_b, &params);
        assert_eq!(pairs.len(), 6); // 2 * 3 = 6 pairs
    }

    #[test]
    fn test_find_candidate_pairs_filtered() {
        // Create windows with shared content
        let windows_a = vec![
            create_test_window(1, 0, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            create_test_window(1, 1, vec![100, 101, 102, 103, 104, 105, 106, 107, 108, 109]),
        ];
        let windows_b = vec![
            create_test_window(2, 0, vec![1, 2, 3, 4, 5, 200, 201, 202, 203, 204]), // Shares shingles with A[0]
            create_test_window(2, 1, vec![300, 301, 302, 303, 304, 305, 306, 307, 308, 309]), // No shared shingles
        ];

        let params = ComparisonParams {
            ngram_size: 3,
            min_shared_shingles: 2,
            brute_force: false,
            ..Default::default()
        };

        let pairs = find_candidate_pairs(&windows_a, &windows_b, &params);

        // Only (0, 0) should be a candidate because they share [1,2,3], [2,3,4], [3,4,5]
        assert!(!pairs.is_empty());
        assert!(pairs.contains(&(0, 0)));
        assert!(!pairs.contains(&(1, 1))); // No shared shingles
    }

}
