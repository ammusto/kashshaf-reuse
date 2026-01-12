//! Criterion benchmarks for Smith-Waterman alignment.
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kashshaf_reuse::align::align_lemma_sequences;
use kashshaf_reuse::models::ComparisonParams;

fn bench_alignment(c: &mut Criterion) {
    let params = ComparisonParams::default();

    // Generate test sequences of different sizes
    let sizes = [100, 275, 500];

    let mut group = c.benchmark_group("smith_waterman");

    for size in sizes {
        // Identical sequences (best case - high score)
        let seq: Vec<u32> = (0..size as u32).collect();

        group.bench_with_input(BenchmarkId::new("identical", size), &size, |b, _| {
            b.iter(|| align_lemma_sequences(black_box(&seq), black_box(&seq), &params))
        });

        // 70% match (typical case)
        let seq_a: Vec<u32> = (0..size as u32).collect();
        let seq_b: Vec<u32> = (0..size as u32)
            .map(|i| if i % 10 < 7 { i } else { i + 10000 })
            .collect();

        group.bench_with_input(BenchmarkId::new("70pct_match", size), &size, |b, _| {
            b.iter(|| align_lemma_sequences(black_box(&seq_a), black_box(&seq_b), &params))
        });

        // 50% match
        let seq_half: Vec<u32> = (0..size as u32)
            .map(|i| if i % 2 == 0 { i } else { i + 10000 })
            .collect();

        group.bench_with_input(BenchmarkId::new("50pct_match", size), &size, |b, _| {
            b.iter(|| align_lemma_sequences(black_box(&seq), black_box(&seq_half), &params))
        });

        // No match (worst case for DP fill, but quick traceback)
        let seq_nomatch: Vec<u32> = (10000..10000 + size as u32).collect();

        group.bench_with_input(BenchmarkId::new("no_match", size), &size, |b, _| {
            b.iter(|| align_lemma_sequences(black_box(&seq), black_box(&seq_nomatch), &params))
        });
    }

    group.finish();
}

fn bench_windowing(c: &mut Criterion) {
    use kashshaf_reuse::models::{BookLemmaStream, PageLemmas};
    use kashshaf_reuse::window::generate_windows;

    let params = ComparisonParams::default();

    let mut group = c.benchmark_group("windowing");

    // Create streams of different sizes
    let sizes = [1000, 10000, 100000];

    for size in sizes {
        let lemmas: Vec<u32> = (0..size as u32).collect();
        let stream = BookLemmaStream {
            book_id: 1,
            total_tokens: size,
            pages: vec![PageLemmas {
                part_index: 1,
                page_id: 1,
                lemma_ids: lemmas,
            }],
        };

        group.bench_with_input(BenchmarkId::new("generate", size), &size, |b, _| {
            b.iter(|| generate_windows(black_box(&stream), &params))
        });
    }

    group.finish();
}

fn bench_filtering(c: &mut Criterion) {
    use kashshaf_reuse::filter::find_candidate_pairs;
    use kashshaf_reuse::models::Window;

    let params = ComparisonParams::default();

    let mut group = c.benchmark_group("filtering");

    // Create sets of windows with varying overlap
    let window_counts = [10, 50, 100];

    for count in window_counts {
        // Create windows with some shared content
        let windows_a: Vec<Window> = (0..count)
            .map(|i| Window {
                book_id: 1,
                window_idx: i as u32,
                global_start: i * 50,
                global_end: i * 50 + 275,
                start_page: (1, 1),
                start_offset: 0,
                end_page: (1, 1),
                end_offset: 0,
                lemma_ids: (i * 50..i * 50 + 275).map(|x| x as u32).collect(),
            })
            .collect();

        // Windows with partial overlap
        let windows_b: Vec<Window> = (0..count)
            .map(|i| Window {
                book_id: 2,
                window_idx: i as u32,
                global_start: i * 50,
                global_end: i * 50 + 275,
                start_page: (1, 1),
                start_offset: 0,
                end_page: (1, 1),
                end_offset: 0,
                // Half matching, half different
                lemma_ids: (i * 50..i * 50 + 275)
                    .map(|x| {
                        if x % 2 == 0 {
                            x as u32
                        } else {
                            x as u32 + 100000
                        }
                    })
                    .collect(),
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("find_pairs", count),
            &count,
            |b, _| {
                b.iter(|| {
                    find_candidate_pairs(
                        black_box(&windows_a),
                        black_box(&windows_b),
                        &params,
                    )
                })
            },
        );
    }

    group.finish();
}

fn bench_shingling(c: &mut Criterion) {
    use kashshaf_reuse::filter::generate_shingles;

    let mut group = c.benchmark_group("shingling");

    let sizes = [100, 275, 500];

    for size in sizes {
        let seq: Vec<u32> = (0..size as u32).collect();

        group.bench_with_input(BenchmarkId::new("generate", size), &size, |b, _| {
            b.iter(|| generate_shingles(black_box(&seq), 5))
        });
    }

    group.finish();
}

fn bench_merging(c: &mut Criterion) {
    use kashshaf_reuse::merge::merge_overlapping_edges;
    use kashshaf_reuse::models::ReuseEdge;

    let mut group = c.benchmark_group("merging");

    let edge_counts = [10, 100, 1000];

    for count in edge_counts {
        // Create overlapping edges
        let edges: Vec<ReuseEdge> = (0..count)
            .map(|i| ReuseEdge {
                id: i as u64,
                source_book_id: 1,
                source_start_page: (1, 1),
                source_start_offset: 0,
                source_end_page: (1, 1),
                source_end_offset: 0,
                source_global_start: i * 40,
                source_global_end: i * 40 + 100,
                target_book_id: 2,
                target_start_page: (1, 1),
                target_start_offset: 0,
                target_end_page: (1, 1),
                target_end_offset: 0,
                target_global_start: i * 40,
                target_global_end: i * 40 + 100,
                aligned_length: 100,
                lemma_matches: 80,
                gaps: 5,
                lemma_similarity: 0.8,
            })
            .collect();

        group.bench_with_input(BenchmarkId::new("merge", count), &count, |b, _| {
            b.iter(|| merge_overlapping_edges(black_box(edges.clone())))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_alignment,
    bench_windowing,
    bench_filtering,
    bench_shingling,
    bench_merging
);
criterion_main!(benches);
