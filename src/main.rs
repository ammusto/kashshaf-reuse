//! Kashshaf Text Reuse Detection Pipeline
//!
//! High-performance text reuse detection for premodern Arabic texts.
//! Compares lemma ID sequences to handle morphological variation automatically.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

mod align;
mod compare;
mod db;
mod extract;
mod filter;
mod merge;
mod models;
mod output;
mod window;

use db::{load_book_info, load_corpus_stats};
use models::{ComparisonParams, MatchMode};
use output::{
    print_edges, print_edges_with_text, print_summary, print_summary_with_text,
    write_csv_file, write_csv_with_text_file, write_json_file, write_json_with_text_file,
    write_viewer_html_file,
};

#[derive(Parser)]
#[command(name = "kashshaf-reuse")]
#[command(about = "High-performance text reuse detection for Arabic texts")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Output format for comparison results
#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    /// JSON file with optional text reconstruction
    Json,
    /// CSV file
    Csv,
    /// Self-contained HTML viewer with embedded React app
    Viewer,
}

/// Matching mode for alignment (CLI version, mirrors models::MatchMode)
#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliMatchMode {
    /// Only count lemma matches
    Lemma,
    /// Only count root matches (ignoring lemma)
    Root,
    /// Lemma match = full score, root-only match = partial score (default)
    Combined,
}

impl From<CliMatchMode> for MatchMode {
    fn from(mode: CliMatchMode) -> Self {
        match mode {
            CliMatchMode::Lemma => MatchMode::Lemma,
            CliMatchMode::Root => MatchMode::Root,
            CliMatchMode::Combined => MatchMode::Combined,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Compare two books for text reuse
    ///
    /// All parameters default to scholar-grade settings from ComparisonParams::default().
    /// Override any parameter explicitly to customize behavior.
    Compare {
        /// Path to corpus.db
        #[arg(long)]
        corpus_db: PathBuf,

        /// First book ID
        #[arg(long)]
        book_a: u32,

        /// Second book ID
        #[arg(long)]
        book_b: u32,

        /// Output file path (extension determines format, or use --format)
        #[arg(long)]
        output: PathBuf,

        /// Output format: json, csv, or viewer (HTML with embedded React app)
        #[arg(long, value_enum, default_value = "json")]
        format: OutputFormat,

        /// Also output CSV file (derived from output path)
        #[arg(long)]
        csv: bool,

        /// Include reconstructed Arabic text in output
        #[arg(long, default_value = "true")]
        include_text: bool,

        /// Number of context tokens before/after each match
        #[arg(long, default_value = "30")]
        context_tokens: usize,

        // === Parameters that inherit from ComparisonParams::default() ===
        // All use Option<T> so we can detect "user didn't specify" vs "user set explicitly"

        /// Window size in tokens [default: 275]
        #[arg(long)]
        window_size: Option<usize>,

        /// Stride between windows [default: 60]
        #[arg(long)]
        stride: Option<usize>,

        /// N-gram size for filtering [default: 5]
        #[arg(long)]
        ngram_size: Option<usize>,

        /// Minimum shared shingles [default: 3]
        #[arg(long)]
        min_shared_shingles: Option<usize>,

        /// Minimum aligned length [default: 10]
        #[arg(long)]
        min_length: Option<usize>,

        /// Minimum similarity ratio - legacy, prefer three-metric filters [default: 0.0]
        #[arg(long)]
        min_similarity: Option<f32>,

        /// Match score for alignment [default: 2]
        #[arg(long)]
        match_score: Option<i32>,

        /// Mismatch penalty for alignment [default: -1]
        #[arg(long)]
        mismatch_penalty: Option<i32>,

        /// Gap penalty for alignment [default: -1]
        #[arg(long)]
        gap_penalty: Option<i32>,

        /// Skip filtering, compare all pairs (slower but thorough)
        #[arg(long)]
        brute_force: bool,

        /// Matching mode [default: combined]
        #[arg(long, value_enum)]
        mode: Option<CliMatchMode>,

        /// Score for lemma match (used in combined mode) [default: 2]
        #[arg(long)]
        lemma_score: Option<i32>,

        /// Score for root-only match (same root, different lemma) [default: 1]
        #[arg(long)]
        root_score: Option<i32>,

        /// Enable document-internal IDF weighting for alignment scoring [default: true]
        #[arg(long, action = clap::ArgAction::Set)]
        use_weights: Option<bool>,

        /// Filter by weighted similarity (IDF-weighted informational density)
        #[arg(long)]
        min_weighted_similarity: Option<f32>,

        /// Filter by core similarity (quotation exactness) [default: 0.85]
        #[arg(long)]
        min_core_similarity: Option<f32>,

        /// Filter by span coverage (reuse vs padding) [default: 0.30]
        #[arg(long)]
        min_span_coverage: Option<f32>,

        /// Filter by content weight (avg IDF of matched lemmas) [default: 1.10]
        #[arg(long)]
        min_content_weight: Option<f32>,

        /// Filter by lexical diversity (unique lemmas / matches) [default: 0.55]
        /// Low diversity indicates formulaic content like isnād phrases
        #[arg(long)]
        min_lexical_diversity: Option<f32>,

        /// Disable all metric filters (for exploratory analysis)
        #[arg(long)]
        no_filters: bool,

        /// Suppress progress output
        #[arg(long)]
        quiet: bool,

        /// Print first N edges to console
        #[arg(long)]
        show_edges: Option<usize>,
    },

    /// Show corpus statistics
    Stats {
        /// Path to corpus.db
        #[arg(long)]
        corpus_db: PathBuf,
    },

    /// Show book information
    Info {
        /// Path to corpus.db
        #[arg(long)]
        corpus_db: PathBuf,

        /// Book ID
        #[arg(long)]
        book_id: u32,

        /// Show individual pages
        #[arg(long)]
        show_pages: bool,
    },

    /// Benchmark alignment performance
    Benchmark {
        /// Number of alignment iterations
        #[arg(long, default_value = "1000")]
        iterations: usize,

        /// Sequence size
        #[arg(long, default_value = "275")]
        size: usize,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compare {
            corpus_db,
            book_a,
            book_b,
            output,
            format,
            csv,
            include_text,
            context_tokens,
            window_size,
            stride,
            ngram_size,
            min_shared_shingles,
            min_length,
            min_similarity,
            match_score,
            mismatch_penalty,
            gap_penalty,
            brute_force,
            mode,
            lemma_score,
            root_score,
            use_weights,
            min_weighted_similarity,
            min_core_similarity,
            min_span_coverage,
            min_content_weight,
            min_lexical_diversity,
            no_filters,
            quiet,
            show_edges,
        } => {
            // Start with library defaults (scholar-grade settings)
            let defaults = ComparisonParams::default();

            // Build params by overlaying user-specified values onto defaults
            let params = ComparisonParams {
                window_size: window_size.unwrap_or(defaults.window_size),
                stride: stride.unwrap_or(defaults.stride),
                ngram_size: ngram_size.unwrap_or(defaults.ngram_size),
                min_shared_shingles: min_shared_shingles.unwrap_or(defaults.min_shared_shingles),
                min_length: min_length.unwrap_or(defaults.min_length),
                min_similarity: min_similarity.unwrap_or(defaults.min_similarity),
                match_score: match_score.unwrap_or(defaults.match_score),
                mismatch_penalty: mismatch_penalty.unwrap_or(defaults.mismatch_penalty),
                gap_penalty: gap_penalty.unwrap_or(defaults.gap_penalty),
                brute_force,
                mode: mode.map(MatchMode::from).unwrap_or(defaults.mode),
                lemma_score: lemma_score.unwrap_or(defaults.lemma_score),
                root_score: root_score.unwrap_or(defaults.root_score),
                use_weights: use_weights.unwrap_or(defaults.use_weights),
                min_weighted_similarity: min_weighted_similarity.or(defaults.min_weighted_similarity),
                // Metric filters: no_filters is explicit in params, effective_* methods handle it
                no_filters,
                min_core_similarity: min_core_similarity.or(defaults.min_core_similarity),
                min_span_coverage: min_span_coverage.or(defaults.min_span_coverage),
                min_content_weight: min_content_weight.or(defaults.min_content_weight),
                min_lexical_diversity: min_lexical_diversity.or(defaults.min_lexical_diversity),
            };

            // Determine if we need text reconstruction
            let need_text = include_text || matches!(format, OutputFormat::Viewer);

            if need_text {
                // Use enhanced comparison with text reconstruction
                let result = compare::compare_books_with_text(
                    book_a,
                    book_b,
                    &corpus_db,
                    &params,
                    context_tokens,
                    !quiet,
                )?;

                // Write output based on format
                match format {
                    OutputFormat::Json => {
                        write_json_with_text_file(&result, &output)?;
                    }
                    OutputFormat::Csv => {
                        write_csv_with_text_file(&result.edges, &output)?;
                    }
                    OutputFormat::Viewer => {
                        let html_output = output.with_extension("html");
                        write_viewer_html_file(&result, &html_output)?;
                        if !quiet {
                            eprintln!("Viewer output: {}", html_output.display());
                        }
                    }
                }

                // Also output CSV if requested (and not already CSV format)
                if csv && !matches!(format, OutputFormat::Csv) {
                    let csv_path = output.with_extension("csv");
                    write_csv_with_text_file(&result.edges, &csv_path)?;
                    if !quiet {
                        eprintln!("CSV output: {}", csv_path.display());
                    }
                }

                // Print summary
                if !quiet {
                    print_summary_with_text(&result);
                    eprintln!("\nOutput: {}", output.display());
                }

                // Show edges if requested
                if let Some(limit) = show_edges {
                    println!("\n=== Sample Edges ===");
                    print_edges_with_text(&result.edges, Some(limit));
                }
            } else {
                // Use standard comparison without text
                let result = compare::compare_books(book_a, book_b, &corpus_db, &params, !quiet)?;

                // Write output
                match format {
                    OutputFormat::Json => {
                        write_json_file(&result, &output)?;
                    }
                    OutputFormat::Csv => {
                        write_csv_file(&result.edges, &output)?;
                    }
                    OutputFormat::Viewer => {
                        // This shouldn't happen because need_text would be true
                        eprintln!("Warning: Viewer format requires text. Falling back to JSON.");
                        write_json_file(&result, &output)?;
                    }
                }

                // Write CSV if requested
                if csv && !matches!(format, OutputFormat::Csv) {
                    let csv_path = output.with_extension("csv");
                    write_csv_file(&result.edges, &csv_path)?;
                    if !quiet {
                        eprintln!("CSV output: {}", csv_path.display());
                    }
                }

                // Print summary
                if !quiet {
                    print_summary(&result);
                    eprintln!("\nOutput: {}", output.display());
                }

                // Show edges if requested
                if let Some(limit) = show_edges {
                    println!("\n=== Sample Edges ===");
                    print_edges(&result.edges, Some(limit));
                }
            }
        }

        Commands::Stats { corpus_db } => {
            let stats = load_corpus_stats(&corpus_db)?;

            println!("=== Corpus Statistics ===");
            println!("Total books: {}", stats.total_books);
            println!("Total pages: {}", stats.total_pages);
            println!("Total tokens: {}", stats.total_tokens);
            println!("Unique lemmas: {}", stats.unique_lemmas);
            println!("Unique roots: {}", stats.unique_roots);
            println!("Token definitions: {}", stats.token_definitions);
        }

        Commands::Info {
            corpus_db,
            book_id,
            show_pages,
        } => {
            let info = load_book_info(&corpus_db, book_id)?;

            println!("=== Book {} ===", info.book_id);
            println!("Pages: {}", info.page_count);
            println!("Total tokens: {}", info.total_tokens);
            println!("Unique lemmas: {}", info.unique_lemmas);
            println!(
                "Avg tokens/page: {:.1}",
                info.total_tokens as f64 / info.page_count as f64
            );

            if show_pages {
                println!("\n=== Pages ===");
                for page in &info.pages {
                    let label = page
                        .page_number
                        .as_deref()
                        .or(page.part_label.as_deref())
                        .unwrap_or("-");
                    println!(
                        "  Part {}, Page {} ({}): {} tokens",
                        page.part_index, page.page_id, label, page.token_count
                    );
                }
            }
        }

        Commands::Benchmark { iterations, size } => {
            run_benchmark(iterations, size);
        }
    }

    Ok(())
}

/// Run alignment benchmark to measure performance.
fn run_benchmark(iterations: usize, size: usize) {
    use std::time::Instant;

    println!("=== Alignment Benchmark ===");
    println!("Iterations: {}", iterations);
    println!("Sequence size: {}", size);

    let params = ComparisonParams::default();

    // Create test sequences
    let seq_identical: Vec<u32> = (0..size as u32).collect();
    let seq_partial: Vec<u32> = (0..size as u32)
        .map(|i| if i % 10 < 7 { i } else { i + 10000 })
        .collect();
    let seq_no_match: Vec<u32> = (10000..10000 + size as u32).collect();

    // Benchmark identical sequences
    println!("\nIdentical sequences:");
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = align::align_lemma_sequences(&seq_identical, &seq_identical, &params);
    }
    let elapsed = start.elapsed();
    let per_alignment = elapsed.as_secs_f64() / iterations as f64;
    let alignments_per_sec = 1.0 / per_alignment;
    println!("  Total time: {:.3}s", elapsed.as_secs_f64());
    println!("  Per alignment: {:.3}ms", per_alignment * 1000.0);
    println!("  Alignments/sec: {:.0}", alignments_per_sec);

    // Benchmark partial match
    println!("\n70% match sequences:");
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = align::align_lemma_sequences(&seq_identical, &seq_partial, &params);
    }
    let elapsed = start.elapsed();
    let per_alignment = elapsed.as_secs_f64() / iterations as f64;
    let alignments_per_sec = 1.0 / per_alignment;
    println!("  Total time: {:.3}s", elapsed.as_secs_f64());
    println!("  Per alignment: {:.3}ms", per_alignment * 1000.0);
    println!("  Alignments/sec: {:.0}", alignments_per_sec);

    // Benchmark no match (quick reject)
    println!("\nNo match sequences:");
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = align::align_lemma_sequences(&seq_identical, &seq_no_match, &params);
    }
    let elapsed = start.elapsed();
    let per_alignment = elapsed.as_secs_f64() / iterations as f64;
    let alignments_per_sec = 1.0 / per_alignment;
    println!("  Total time: {:.3}s", elapsed.as_secs_f64());
    println!("  Per alignment: {:.3}ms", per_alignment * 1000.0);
    println!("  Alignments/sec: {:.0}", alignments_per_sec);
}
