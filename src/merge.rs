//! Merge overlapping reuse edges into maximal spans.
//!
//! When windows overlap, the same text reuse can be detected multiple times.
//! This module merges these overlapping detections into single, maximal spans.

use crate::models::ReuseEdge;

/// Merge overlapping edges into maximal spans.
///
/// Edges are considered overlapping if they involve the same book pair
/// and their source/target regions overlap significantly.
pub fn merge_overlapping_edges(mut edges: Vec<ReuseEdge>) -> Vec<ReuseEdge> {
    if edges.len() <= 1 {
        return edges;
    }

    // Sort by source position
    edges.sort_by_key(|e| {
        (
            e.source_book_id,
            e.target_book_id,
            e.source_global_start,
            e.target_global_start,
        )
    });

    let mut merged: Vec<ReuseEdge> = Vec::new();

    for edge in edges {
        let should_merge = if let Some(last) = merged.last() {
            last.source_book_id == edge.source_book_id
                && last.target_book_id == edge.target_book_id
                && edges_overlap(last, &edge)
        } else {
            false
        };

        if should_merge {
            let last = merged.last_mut().unwrap();
            *last = merge_two_edges(last, &edge);
        } else {
            merged.push(edge);
        }
    }

    merged
}

/// Check if two edges overlap in both source and target positions.
fn edges_overlap(a: &ReuseEdge, b: &ReuseEdge) -> bool {
    // Check source overlap
    let source_overlap = ranges_overlap(
        a.source_global_start,
        a.source_global_end,
        b.source_global_start,
        b.source_global_end,
    );

    // Check target overlap
    let target_overlap = ranges_overlap(
        a.target_global_start,
        a.target_global_end,
        b.target_global_start,
        b.target_global_end,
    );

    source_overlap && target_overlap
}

/// Check if two ranges overlap.
#[inline]
fn ranges_overlap(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> bool {
    start_a < end_b && start_b < end_a
}

/// Merge two overlapping edges into one.
fn merge_two_edges(a: &ReuseEdge, b: &ReuseEdge) -> ReuseEdge {
    // Calculate merged source range
    let source_global_start = a.source_global_start.min(b.source_global_start);
    let source_global_end = a.source_global_end.max(b.source_global_end);

    // Calculate merged target range
    let target_global_start = a.target_global_start.min(b.target_global_start);
    let target_global_end = a.target_global_end.max(b.target_global_end);

    // Choose page info based on which edge defines the boundary
    let (source_start_page, source_start_offset) = if a.source_global_start <= b.source_global_start
    {
        (a.source_start_page, a.source_start_offset)
    } else {
        (b.source_start_page, b.source_start_offset)
    };

    let (source_end_page, source_end_offset) = if a.source_global_end >= b.source_global_end {
        (a.source_end_page, a.source_end_offset)
    } else {
        (b.source_end_page, b.source_end_offset)
    };

    let (target_start_page, target_start_offset) = if a.target_global_start <= b.target_global_start
    {
        (a.target_start_page, a.target_start_offset)
    } else {
        (b.target_start_page, b.target_start_offset)
    };

    let (target_end_page, target_end_offset) = if a.target_global_end >= b.target_global_end {
        (a.target_end_page, a.target_end_offset)
    } else {
        (b.target_end_page, b.target_end_offset)
    };

    // Combine statistics
    let aligned_length = (source_global_end - source_global_start) as u32;

    // For lemma_matches and gaps, we need to account for overlap
    // This is an approximation since we don't have the original alignments
    let overlap_source = calculate_overlap_size(
        a.source_global_start,
        a.source_global_end,
        b.source_global_start,
        b.source_global_end,
    );

    let combined_matches = a.lemma_matches + b.lemma_matches;
    let overlap_matches = (overlap_source as f32 * a.lemma_similarity) as u32;
    let lemma_matches = combined_matches.saturating_sub(overlap_matches);

    // Combine substitutions similarly
    let combined_subs = a.substitutions + b.substitutions;
    let overlap_subs = if a.aligned_length > 0 {
        (overlap_source as f32 * (a.substitutions as f32 / a.aligned_length as f32)) as u32
    } else {
        0
    };
    let substitutions = combined_subs.saturating_sub(overlap_subs);

    // Combine root_only_matches similarly
    let combined_root_only = a.root_only_matches + b.root_only_matches;
    let overlap_root_only = if a.aligned_length > 0 {
        (overlap_source as f32 * (a.root_only_matches as f32 / a.aligned_length as f32)) as u32
    } else {
        0
    };
    let root_only_matches = combined_root_only.saturating_sub(overlap_root_only);

    let combined_gaps = a.gaps + b.gaps;
    let gaps = combined_gaps / 2; // Rough estimate

    // Calculate three orthogonal metrics
    let match_sub_total = lemma_matches + substitutions;
    let core_similarity = if match_sub_total > 0 {
        lemma_matches as f32 / match_sub_total as f32
    } else {
        0.0
    };

    let span_coverage = if aligned_length > 0 {
        match_sub_total as f32 / aligned_length as f32
    } else {
        0.0
    };

    // Average content weight from both edges
    let content_weight = (a.content_weight + b.content_weight) / 2.0;

    // Legacy metrics
    let lemma_similarity = if aligned_length > 0 {
        lemma_matches as f32 / aligned_length as f32
    } else {
        0.0
    };

    let combined_similarity = if aligned_length > 0 {
        (lemma_matches as f32 + 0.5 * root_only_matches as f32) / aligned_length as f32
    } else {
        0.0
    };

    ReuseEdge {
        id: a.id, // Keep the first edge's ID
        source_book_id: a.source_book_id,
        source_start_page,
        source_start_offset,
        source_end_page,
        source_end_offset,
        source_global_start,
        source_global_end,
        target_book_id: a.target_book_id,
        target_start_page,
        target_start_offset,
        target_end_page,
        target_end_offset,
        target_global_start,
        target_global_end,
        aligned_length,
        lemma_matches,
        substitutions,
        root_only_matches,
        gaps,
        core_similarity,
        span_coverage,
        content_weight,
        lemma_similarity,
        combined_similarity,
        // For merged edges, we average the weighted metrics
        weighted_similarity: (a.weighted_similarity + b.weighted_similarity) / 2.0,
        avg_match_weight: content_weight,
    }
}

/// Calculate the overlap size between two ranges.
fn calculate_overlap_size(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> usize {
    let overlap_start = start_a.max(start_b);
    let overlap_end = end_a.min(end_b);

    if overlap_start < overlap_end {
        overlap_end - overlap_start
    } else {
        0
    }
}

/// Merge edges that are adjacent (touching but not overlapping).
/// This can be useful for combining edges that were split across window boundaries.
pub fn merge_adjacent_edges(mut edges: Vec<ReuseEdge>, max_gap: usize) -> Vec<ReuseEdge> {
    if edges.len() <= 1 {
        return edges;
    }

    // Sort by source position
    edges.sort_by_key(|e| {
        (
            e.source_book_id,
            e.target_book_id,
            e.source_global_start,
        )
    });

    let mut merged: Vec<ReuseEdge> = Vec::new();

    for edge in edges {
        let should_merge = if let Some(last) = merged.last() {
            last.source_book_id == edge.source_book_id
                && last.target_book_id == edge.target_book_id
                && edges_adjacent(last, &edge, max_gap)
        } else {
            false
        };

        if should_merge {
            let last = merged.last_mut().unwrap();
            *last = merge_two_edges(last, &edge);
        } else {
            merged.push(edge);
        }
    }

    merged
}

/// Check if two edges are adjacent (within max_gap tokens).
fn edges_adjacent(a: &ReuseEdge, b: &ReuseEdge, max_gap: usize) -> bool {
    // Check source adjacency
    let source_gap = if b.source_global_start >= a.source_global_end {
        b.source_global_start - a.source_global_end
    } else if a.source_global_start >= b.source_global_end {
        a.source_global_start - b.source_global_end
    } else {
        0 // They overlap
    };

    // Check target adjacency
    let target_gap = if b.target_global_start >= a.target_global_end {
        b.target_global_start - a.target_global_end
    } else if a.target_global_start >= b.target_global_end {
        a.target_global_start - b.target_global_end
    } else {
        0 // They overlap
    };

    source_gap <= max_gap && target_gap <= max_gap
}

/// Filter out edges that are subsumed by larger edges.
pub fn remove_subsumed_edges(mut edges: Vec<ReuseEdge>) -> Vec<ReuseEdge> {
    if edges.len() <= 1 {
        return edges;
    }

    // Sort by aligned length descending
    edges.sort_by(|a, b| b.aligned_length.cmp(&a.aligned_length));

    let mut retained: Vec<ReuseEdge> = Vec::new();

    for edge in edges {
        let is_subsumed = retained.iter().any(|existing| {
            existing.source_book_id == edge.source_book_id
                && existing.target_book_id == edge.target_book_id
                && existing.source_global_start <= edge.source_global_start
                && existing.source_global_end >= edge.source_global_end
                && existing.target_global_start <= edge.target_global_start
                && existing.target_global_end >= edge.target_global_end
        });

        if !is_subsumed {
            retained.push(edge);
        }
    }

    // Re-sort by position
    retained.sort_by_key(|e| {
        (
            e.source_book_id,
            e.target_book_id,
            e.source_global_start,
        )
    });

    retained
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_edge(
        id: u64,
        source_start: usize,
        source_end: usize,
        target_start: usize,
        target_end: usize,
    ) -> ReuseEdge {
        let aligned_length = (source_end - source_start) as u32;
        ReuseEdge {
            id,
            source_book_id: 1,
            source_start_page: (1, 1),
            source_start_offset: 0,
            source_end_page: (1, 1),
            source_end_offset: 0,
            source_global_start: source_start,
            source_global_end: source_end,
            target_book_id: 2,
            target_start_page: (1, 1),
            target_start_offset: 0,
            target_end_page: (1, 1),
            target_end_offset: 0,
            target_global_start: target_start,
            target_global_end: target_end,
            aligned_length,
            lemma_matches: aligned_length,
            substitutions: 0,
            root_only_matches: 0,
            gaps: 0,
            core_similarity: 1.0,
            span_coverage: 1.0,
            content_weight: 1.0,
            lemma_similarity: 1.0,
            combined_similarity: 1.0,
            weighted_similarity: 1.0,
            avg_match_weight: 1.0,
        }
    }

    #[test]
    fn test_no_merge_needed() {
        let edges = vec![
            create_edge(1, 0, 100, 0, 100),
            create_edge(2, 200, 300, 200, 300),
        ];

        let merged = merge_overlapping_edges(edges);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_overlapping() {
        let edges = vec![
            create_edge(1, 0, 100, 0, 100),
            create_edge(2, 50, 150, 50, 150),
        ];

        let merged = merge_overlapping_edges(edges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source_global_start, 0);
        assert_eq!(merged[0].source_global_end, 150);
    }

    #[test]
    fn test_merge_multiple_overlapping() {
        let edges = vec![
            create_edge(1, 0, 100, 0, 100),
            create_edge(2, 50, 150, 50, 150),
            create_edge(3, 100, 200, 100, 200),
        ];

        let merged = merge_overlapping_edges(edges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source_global_start, 0);
        assert_eq!(merged[0].source_global_end, 200);
    }

    #[test]
    fn test_no_merge_when_only_source_overlaps() {
        let edges = vec![
            create_edge(1, 0, 100, 0, 100),
            create_edge(2, 50, 150, 200, 300), // Target doesn't overlap
        ];

        let merged = merge_overlapping_edges(edges);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_ranges_overlap() {
        assert!(ranges_overlap(0, 100, 50, 150));
        assert!(ranges_overlap(50, 150, 0, 100));
        assert!(!ranges_overlap(0, 100, 100, 200));
        assert!(!ranges_overlap(0, 100, 200, 300));
    }

    #[test]
    fn test_empty_edges() {
        let edges: Vec<ReuseEdge> = vec![];
        let merged = merge_overlapping_edges(edges);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_single_edge() {
        let edges = vec![create_edge(1, 0, 100, 0, 100)];
        let merged = merge_overlapping_edges(edges);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_merge_adjacent() {
        let edges = vec![
            create_edge(1, 0, 100, 0, 100),
            create_edge(2, 105, 200, 105, 200), // Gap of 5
        ];

        let merged = merge_adjacent_edges(edges, 10);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_no_merge_adjacent_too_far() {
        let edges = vec![
            create_edge(1, 0, 100, 0, 100),
            create_edge(2, 150, 250, 150, 250), // Gap of 50
        ];

        let merged = merge_adjacent_edges(edges, 10);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_remove_subsumed() {
        let edges = vec![
            create_edge(1, 0, 200, 0, 200),     // Larger
            create_edge(2, 50, 150, 50, 150),   // Subsumed
        ];

        let retained = remove_subsumed_edges(edges);
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].id, 1);
    }
}
