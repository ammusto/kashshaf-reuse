//! Output formatting for comparison results (JSON, CSV, HTML viewer).

use crate::models::{ComparisonResult, ComparisonResultWithText, ReuseEdge, ReuseEdgeWithText};
use std::io::{self, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Write comparison result as JSON.
pub fn write_json<W: Write>(result: &ComparisonResult, writer: &mut W) -> Result<(), OutputError> {
    let json = serde_json::to_string_pretty(result)?;
    writer.write_all(json.as_bytes())?;
    Ok(())
}

/// Write comparison result as JSON to a file.
pub fn write_json_file(result: &ComparisonResult, path: &Path) -> Result<(), OutputError> {
    let mut file = std::fs::File::create(path)?;
    write_json(result, &mut file)
}

/// Write edges as CSV.
pub fn write_csv<W: Write>(edges: &[ReuseEdge], writer: &mut W) -> Result<(), OutputError> {
    // Write header
    writeln!(
        writer,
        "id,source_book_id,source_start_part,source_start_page,source_start_offset,\
         source_end_part,source_end_page,source_end_offset,source_global_start,source_global_end,\
         target_book_id,target_start_part,target_start_page,target_start_offset,\
         target_end_part,target_end_page,target_end_offset,target_global_start,target_global_end,\
         aligned_length,lemma_matches,substitutions,root_only_matches,gaps,\
         core_similarity,span_coverage,content_weight,\
         lemma_similarity,combined_similarity,weighted_similarity"
    )?;

    // Write rows
    for edge in edges {
        writeln!(
            writer,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            edge.id,
            edge.source_book_id,
            edge.source_start_page.0,
            edge.source_start_page.1,
            edge.source_start_offset,
            edge.source_end_page.0,
            edge.source_end_page.1,
            edge.source_end_offset,
            edge.source_global_start,
            edge.source_global_end,
            edge.target_book_id,
            edge.target_start_page.0,
            edge.target_start_page.1,
            edge.target_start_offset,
            edge.target_end_page.0,
            edge.target_end_page.1,
            edge.target_end_offset,
            edge.target_global_start,
            edge.target_global_end,
            edge.aligned_length,
            edge.lemma_matches,
            edge.substitutions,
            edge.root_only_matches,
            edge.gaps,
            edge.core_similarity,
            edge.span_coverage,
            edge.content_weight,
            edge.lemma_similarity,
            edge.combined_similarity,
            edge.weighted_similarity
        )?;
    }

    Ok(())
}

/// Write edges as CSV to a file.
pub fn write_csv_file(edges: &[ReuseEdge], path: &Path) -> Result<(), OutputError> {
    let mut file = std::fs::File::create(path)?;
    write_csv(edges, &mut file)
}

/// Write a summary report to stdout.
pub fn print_summary(result: &ComparisonResult) {
    println!("\n=== Comparison Summary ===");
    println!("Version: {}", result.version);
    println!();
    println!("Book A: {} ({} tokens)", result.book_a.id, result.book_a.token_count);
    println!("Book B: {} ({} tokens)", result.book_b.id, result.book_b.token_count);
    println!();
    println!("Parameters:");
    println!("  Window size: {}", result.parameters.window_size);
    println!("  Stride: {}", result.parameters.stride);
    println!("  N-gram size: {}", result.parameters.ngram_size);
    println!("  Min shared shingles: {}", result.parameters.min_shared_shingles);
    println!("  Min length: {}", result.parameters.min_length);
    println!("  Min similarity: {:.1}%", result.parameters.min_similarity * 100.0);
    println!("  Brute force: {}", result.parameters.brute_force);
    println!();
    println!("Results:");
    println!("  Edges found: {}", result.summary.edge_count);
    println!("  Total aligned tokens: {}", result.summary.total_aligned_tokens);
    println!("  Book A coverage: {:.1}%", result.summary.book_a_coverage * 100.0);
    println!("  Book B coverage: {:.1}%", result.summary.book_b_coverage * 100.0);
    println!("  Average similarity: {:.1}%", result.summary.avg_similarity * 100.0);
}

/// Format a page location as a string.
pub fn format_page_location(part_index: u32, page_id: u32, offset: u32) -> String {
    format!("{}:{}.{}", part_index, page_id, offset)
}

/// Format an edge as a human-readable string.
pub fn format_edge(edge: &ReuseEdge) -> String {
    format!(
        "Edge {}: len={} matches={} subs={} gaps={}\n\
         \x20 Core: {:.1}%  Coverage: {:.1}%  Weight: {:.2}\n\
         \x20 Book {} [{}→{}] ↔ Book {} [{}→{}]",
        edge.id,
        edge.aligned_length,
        edge.lemma_matches,
        edge.substitutions,
        edge.gaps,
        edge.core_similarity * 100.0,
        edge.span_coverage * 100.0,
        edge.content_weight,
        edge.source_book_id,
        format_page_location(
            edge.source_start_page.0,
            edge.source_start_page.1,
            edge.source_start_offset
        ),
        format_page_location(
            edge.source_end_page.0,
            edge.source_end_page.1,
            edge.source_end_offset
        ),
        edge.target_book_id,
        format_page_location(
            edge.target_start_page.0,
            edge.target_start_page.1,
            edge.target_start_offset
        ),
        format_page_location(
            edge.target_end_page.0,
            edge.target_end_page.1,
            edge.target_end_offset
        ),
    )
}

/// Print edges in a human-readable format.
pub fn print_edges(edges: &[ReuseEdge], limit: Option<usize>) {
    let to_print = match limit {
        Some(n) => &edges[..n.min(edges.len())],
        None => edges,
    };

    for edge in to_print {
        println!("{}", format_edge(edge));
    }

    if let Some(n) = limit {
        if edges.len() > n {
            println!("... and {} more edges", edges.len() - n);
        }
    }
}

// ============================================================================
// Enhanced output with text
// ============================================================================

/// Write comparison result with text as JSON.
pub fn write_json_with_text<W: Write>(
    result: &ComparisonResultWithText,
    writer: &mut W,
) -> Result<(), OutputError> {
    let json = serde_json::to_string_pretty(result)?;
    writer.write_all(json.as_bytes())?;
    Ok(())
}

/// Write comparison result with text as JSON to a file.
pub fn write_json_with_text_file(
    result: &ComparisonResultWithText,
    path: &Path,
) -> Result<(), OutputError> {
    let mut file = std::fs::File::create(path)?;
    write_json_with_text(result, &mut file)
}

/// Write edges with text as CSV.
pub fn write_csv_with_text<W: Write>(
    edges: &[ReuseEdgeWithText],
    writer: &mut W,
) -> Result<(), OutputError> {
    // Write header
    writeln!(
        writer,
        "id,source_book_id,source_location,source_global_start,source_global_end,\
         source_text_before,source_text_matched,source_text_after,\
         target_book_id,target_location,target_global_start,target_global_end,\
         target_text_before,target_text_matched,target_text_after,\
         aligned_length,lemma_matches,gaps,similarity"
    )?;

    // Write rows
    for edge in edges {
        writeln!(
            writer,
            "{},{},{:?},{},{},{:?},{:?},{:?},{},{},{},{},{:?},{:?},{:?},{},{},{},{}",
            edge.id,
            edge.source.book_id,
            edge.source.location,
            edge.source.global_range.0,
            edge.source.global_range.1,
            edge.source.text.before,
            edge.source.text.matched,
            edge.source.text.after,
            edge.target.book_id,
            edge.target.location,
            edge.target.global_range.0,
            edge.target.global_range.1,
            edge.target.text.before,
            edge.target.text.matched,
            edge.target.text.after,
            edge.alignment.length,
            edge.alignment.lemma_matches,
            edge.alignment.gaps,
            edge.alignment.similarity
        )?;
    }

    Ok(())
}

/// Write edges with text as CSV to a file.
pub fn write_csv_with_text_file(
    edges: &[ReuseEdgeWithText],
    path: &Path,
) -> Result<(), OutputError> {
    let mut file = std::fs::File::create(path)?;
    write_csv_with_text(edges, &mut file)
}

/// Print edges with text in a human-readable format.
pub fn print_edges_with_text(edges: &[ReuseEdgeWithText], limit: Option<usize>) {
    let to_print = match limit {
        Some(n) => &edges[..n.min(edges.len())],
        None => edges,
    };

    for edge in to_print {
        println!("{}", format_edge_with_text(edge));
    }

    if let Some(n) = limit {
        if edges.len() > n {
            println!("... and {} more edges", edges.len() - n);
        }
    }
}

/// Format an edge with text as a human-readable string.
pub fn format_edge_with_text(edge: &ReuseEdgeWithText) -> String {
    format!(
        "Edge {}: len={} matches={} subs={} gaps={}\n\
         \x20 Core: {:.1}%  Coverage: {:.1}%  Weight: {:.2}\n\
         \x20 Book {} [{}] ↔ Book {} [{}]\n\
         Source: {}\n\
         Target: {}",
        edge.id,
        edge.alignment.length,
        edge.alignment.lemma_matches,
        edge.alignment.substitutions,
        edge.alignment.gaps,
        edge.alignment.core_similarity * 100.0,
        edge.alignment.span_coverage * 100.0,
        edge.alignment.content_weight,
        edge.source.book_id,
        edge.source.location,
        edge.target.book_id,
        edge.target.location,
        truncate_text(&edge.source.text.matched, 100),
        truncate_text(&edge.target.text.matched, 100),
    )
}

/// Truncate text to a maximum length, adding ellipsis if needed.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

/// Print summary for results with text.
pub fn print_summary_with_text(result: &ComparisonResultWithText) {
    println!("\n=== Comparison Summary ===");
    println!("Version: {}", result.version);
    println!("Generated: {}", result.generated_at);
    println!();
    println!(
        "Book A: {} - {} ({} tokens)",
        result.book_a.id,
        if result.book_a.title.is_empty() {
            "(untitled)"
        } else {
            &result.book_a.title
        },
        result.book_a.token_count
    );
    println!(
        "Book B: {} - {} ({} tokens)",
        result.book_b.id,
        if result.book_b.title.is_empty() {
            "(untitled)"
        } else {
            &result.book_b.title
        },
        result.book_b.token_count
    );
    println!();
    println!("Parameters:");
    println!("  Window size: {}", result.parameters.window_size);
    println!("  Stride: {}", result.parameters.stride);
    println!("  N-gram size: {}", result.parameters.ngram_size);
    println!(
        "  Min shared shingles: {}",
        result.parameters.min_shared_shingles
    );
    println!("  Min length: {}", result.parameters.min_length);
    println!(
        "  Min similarity: {:.1}%",
        result.parameters.min_similarity * 100.0
    );
    println!("  Brute force: {}", result.parameters.brute_force);
    println!();
    println!("Results:");
    println!("  Edges found: {}", result.summary.edge_count);
    println!(
        "  Total aligned tokens: {}",
        result.summary.total_aligned_tokens
    );
    println!(
        "  Book A coverage: {:.1}%",
        result.summary.book_a_coverage * 100.0
    );
    println!(
        "  Book B coverage: {:.1}%",
        result.summary.book_b_coverage * 100.0
    );
    println!(
        "  Average similarity: {:.1}%",
        result.summary.avg_similarity * 100.0
    );
}

// ============================================================================
// HTML Viewer generation
// ============================================================================

/// Generate a self-contained HTML viewer for the comparison results.
pub fn generate_viewer_html(result: &ComparisonResultWithText) -> String {
    let data_json = serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string());

    // Escape any </script> tags in the JSON to prevent breaking the HTML
    let escaped_json = data_json.replace("</script>", "<\\/script>");

    let book_a_title = if result.book_a.title.is_empty() {
        format!("Book {}", result.book_a.id)
    } else {
        result.book_a.title.clone()
    };

    let book_b_title = if result.book_b.title.is_empty() {
        format!("Book {}", result.book_b.id)
    } else {
        result.book_b.title.clone()
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en" dir="ltr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Kashshaf Reuse Viewer - {book_a} vs {book_b}</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <script src="https://unpkg.com/react@18/umd/react.production.min.js"></script>
    <script src="https://unpkg.com/react-dom@18/umd/react-dom.production.min.js"></script>
    <script src="https://unpkg.com/@babel/standalone/babel.min.js"></script>
    <style>
        .arabic-text {{
            font-family: 'Amiri', 'Traditional Arabic', 'Scheherazade', serif;
            font-size: 1.1rem;
            line-height: 2;
        }}
        .highlight-match {{
            background-color: #fef08a;
            padding: 2px 4px;
            border-radius: 3px;
        }}
        .context-text {{
            color: #9ca3af;
        }}
        .similarity-high {{ color: #16a34a; }}
        .similarity-medium {{ color: #ca8a04; }}
        .similarity-low {{ color: #dc2626; }}
    </style>
</head>
<body class="bg-gray-50">
    <div id="root"></div>

    <script type="text/javascript">
        window.__COMPARISON_DATA__ = {data_json};
    </script>

    <script type="text/babel">
{viewer_app}
    </script>
</body>
</html>"##,
        book_a = book_a_title,
        book_b = book_b_title,
        data_json = escaped_json,
        viewer_app = VIEWER_APP_CODE,
    )
}

/// Write viewer HTML to a file.
pub fn write_viewer_html_file(
    result: &ComparisonResultWithText,
    path: &Path,
) -> Result<(), OutputError> {
    let html = generate_viewer_html(result);
    std::fs::write(path, html)?;
    Ok(())
}

/// Embedded React viewer application code
const VIEWER_APP_CODE: &str = r##"
const {{ useState, useEffect, useMemo }} = React;

// Main App Component
function App() {{
    const [data, setData] = useState(null);
    const [selectedEdge, setSelectedEdge] = useState(null);
    const [filters, setFilters] = useState({{
        minSimilarity: 0,
        minLength: 0,
        searchText: '',
        sortBy: 'similarity',
        sortDesc: true,
    }});
    const [validations, setValidations] = useState({{}});

    useEffect(() => {{
        setData(window.__COMPARISON_DATA__);
    }}, []);

    const filteredEdges = useMemo(() => {{
        if (!data) return [];

        let edges = data.edges.filter(edge =>
            edge.alignment.similarity >= filters.minSimilarity &&
            edge.alignment.length >= filters.minLength &&
            (filters.searchText === '' ||
                edge.source.text.matched.includes(filters.searchText) ||
                edge.target.text.matched.includes(filters.searchText))
        );

        // Sort
        edges.sort((a, b) => {{
            let cmp = 0;
            switch (filters.sortBy) {{
                case 'similarity':
                    cmp = a.alignment.similarity - b.alignment.similarity;
                    break;
                case 'length':
                    cmp = a.alignment.length - b.alignment.length;
                    break;
                case 'position':
                    cmp = a.source.global_range[0] - b.source.global_range[0];
                    break;
                default:
                    cmp = a.id - b.id;
            }}
            return filters.sortDesc ? -cmp : cmp;
        }});

        return edges;
    }}, [data, filters]);

    if (!data) {{
        return (
            <div className="h-screen flex items-center justify-center">
                <div className="text-gray-500">Loading...</div>
            </div>
        );
    }}

    const validCount = Object.values(validations).filter(v => v === 'valid').length;
    const noiseCount = Object.values(validations).filter(v => v === 'noise').length;

    return (
        <div className="h-screen flex flex-col">
            {{/* Header */}}
            <header className="bg-white border-b px-4 py-3">
                <div className="flex justify-between items-center">
                    <div>
                        <h1 className="text-xl font-bold">Kashshaf Text Reuse Viewer</h1>
                        <p className="text-sm text-gray-600">
                            {{data.book_a.title || `Book ${{data.book_a.id}}`}} vs {{data.book_b.title || `Book ${{data.book_b.id}}`}}
                        </p>
                    </div>
                    <div className="text-right text-sm">
                        <div>{{data.summary.edge_count}} total matches</div>
                        <div className="text-gray-500">
                            Avg similarity: {{(data.summary.avg_similarity * 100).toFixed(1)}}%
                        </div>
                    </div>
                </div>
            </header>

            {{/* Stats Bar */}}
            <div className="bg-gray-100 px-4 py-2 border-b flex gap-6 text-sm">
                <span>Showing: <strong>{{filteredEdges.length}}</strong> matches</span>
                <span className="text-green-600">✓ Valid: {{validCount}}</span>
                <span className="text-red-600">✗ Noise: {{noiseCount}}</span>
                <span className="text-gray-500">
                    Book A coverage: {{(data.summary.book_a_coverage * 100).toFixed(1)}}% |
                    Book B coverage: {{(data.summary.book_b_coverage * 100).toFixed(1)}}%
                </span>
            </div>

            {{/* Filter Bar */}}
            <div className="bg-white px-4 py-2 border-b flex gap-4 items-center text-sm">
                <label className="flex items-center gap-2">
                    Min similarity:
                    <input
                        type="range"
                        min="0"
                        max="100"
                        value={{filters.minSimilarity * 100}}
                        onChange={{e => setFilters(f => ({{ ...f, minSimilarity: e.target.value / 100 }}))}}
                        className="w-24"
                    />
                    <span className="w-12">{{(filters.minSimilarity * 100).toFixed(0)}}%</span>
                </label>
                <label className="flex items-center gap-2">
                    Min length:
                    <input
                        type="number"
                        min="0"
                        value={{filters.minLength}}
                        onChange={{e => setFilters(f => ({{ ...f, minLength: parseInt(e.target.value) || 0 }}))}}
                        className="w-16 border rounded px-2 py-1"
                    />
                </label>
                <label className="flex items-center gap-2">
                    Search:
                    <input
                        type="text"
                        value={{filters.searchText}}
                        onChange={{e => setFilters(f => ({{ ...f, searchText: e.target.value }}))}}
                        placeholder="Arabic text..."
                        className="w-48 border rounded px-2 py-1"
                        dir="rtl"
                    />
                </label>
                <label className="flex items-center gap-2">
                    Sort by:
                    <select
                        value={{filters.sortBy}}
                        onChange={{e => setFilters(f => ({{ ...f, sortBy: e.target.value }}))}}
                        className="border rounded px-2 py-1"
                    >
                        <option value="similarity">Similarity</option>
                        <option value="length">Length</option>
                        <option value="position">Position</option>
                        <option value="id">ID</option>
                    </select>
                </label>
                <button
                    onClick={{() => setFilters(f => ({{ ...f, sortDesc: !f.sortDesc }}))}}
                    className="border rounded px-2 py-1 hover:bg-gray-100"
                >
                    {{filters.sortDesc ? '↓ Desc' : '↑ Asc'}}
                </button>
                <button
                    onClick={{() => {{
                        const validated = filteredEdges.filter(e => validations[e.id]);
                        const csvContent = [
                            ['id', 'validation', 'source_text', 'target_text', 'similarity'].join(','),
                            ...validated.map(e => [
                                e.id,
                                validations[e.id],
                                `"${{e.source.text.matched.replace(/"/g, '""')}}"`,
                                `"${{e.target.text.matched.replace(/"/g, '""')}}"`,
                                e.alignment.similarity
                            ].join(','))
                        ].join('\n');
                        const blob = new Blob([csvContent], {{ type: 'text/csv' }});
                        const url = URL.createObjectURL(blob);
                        const a = document.createElement('a');
                        a.href = url;
                        a.download = 'validated_matches.csv';
                        a.click();
                    }}}}
                    className="ml-auto border rounded px-3 py-1 bg-blue-50 hover:bg-blue-100 text-blue-700"
                >
                    Export Validated
                </button>
            </div>

            {{/* Main Content */}}
            <div className="flex-1 flex overflow-hidden">
                {{/* Match List */}}
                <div className="w-80 border-r overflow-auto bg-white">
                    {{filteredEdges.map(edge => (
                        <div
                            key={{edge.id}}
                            onClick={{() => setSelectedEdge(edge)}}
                            className={{`p-3 border-b cursor-pointer hover:bg-gray-50 ${{
                                selectedEdge?.id === edge.id ? 'bg-blue-50 border-l-4 border-l-blue-500' : ''
                            }}`}}
                        >
                            <div className="flex justify-between items-start">
                                <span className="text-sm text-gray-500">#{{edge.id}}</span>
                                <div className="flex items-center gap-1">
                                    {{validations[edge.id] === 'valid' && (
                                        <span className="text-green-500">✓</span>
                                    )}}
                                    {{validations[edge.id] === 'noise' && (
                                        <span className="text-red-500">✗</span>
                                    )}}
                                    <span className={{`text-sm font-bold ${{
                                        (edge.alignment.core_similarity || edge.alignment.similarity) >= 0.9 ? 'similarity-high' :
                                        (edge.alignment.core_similarity || edge.alignment.similarity) >= 0.7 ? 'similarity-medium' :
                                        'similarity-low'
                                    }}`}}>
                                        {{((edge.alignment.core_similarity || edge.alignment.similarity) * 100).toFixed(0)}}%
                                    </span>
                                </div>
                            </div>
                            <div className="text-sm mt-1 text-gray-600">
                                {{edge.alignment.length}} tok • {{((edge.alignment.span_coverage || 1) * 100).toFixed(0)}}% cov
                            </div>
                            <div
                                className="text-sm text-gray-600 mt-1 truncate arabic-text"
                                dir="rtl"
                                lang="ar"
                            >
                                {{edge.source.text.matched.slice(0, 50)}}...
                            </div>
                        </div>
                    ))}}
                </div>

                {{/* Detail View */}}
                <div className="flex-1 overflow-auto p-4">
                    {{selectedEdge ? (
                        <div>
                            {{/* Header with stats */}}
                            <div className="mb-4 p-3 bg-gray-100 rounded-lg">
                                <div className="flex justify-between items-center mb-3">
                                    <span className="font-bold text-lg">Match #{{selectedEdge.id}}</span>
                                    <div className="flex gap-2">
                                        <button
                                            onClick={{() => setValidations(v => ({{ ...v, [selectedEdge.id]: 'valid' }}))}}
                                            className={{`px-3 py-1 rounded ${{
                                                validations[selectedEdge.id] === 'valid'
                                                    ? 'bg-green-500 text-white'
                                                    : 'bg-gray-200 hover:bg-green-100'
                                            }}`}}
                                        >
                                            ✓ Valid
                                        </button>
                                        <button
                                            onClick={{() => setValidations(v => ({{ ...v, [selectedEdge.id]: 'noise' }}))}}
                                            className={{`px-3 py-1 rounded ${{
                                                validations[selectedEdge.id] === 'noise'
                                                    ? 'bg-red-500 text-white'
                                                    : 'bg-gray-200 hover:bg-red-100'
                                            }}`}}
                                        >
                                            ✗ Noise
                                        </button>
                                    </div>
                                </div>
                                {{/* Three metrics display */}}
                                <div className="grid grid-cols-3 gap-4 mb-3">
                                    <div className="bg-white p-2 rounded text-center">
                                        <div className="text-xs text-gray-500">Core Similarity</div>
                                        <div className={{`text-xl font-bold ${{
                                            (selectedEdge.alignment.core_similarity || 0) >= 0.9 ? 'text-green-600' :
                                            (selectedEdge.alignment.core_similarity || 0) >= 0.7 ? 'text-yellow-600' :
                                            'text-red-600'
                                        }}`}}>
                                            {{((selectedEdge.alignment.core_similarity || 0) * 100).toFixed(1)}}%
                                        </div>
                                        <div className="text-xs text-gray-400">quotation exactness</div>
                                    </div>
                                    <div className="bg-white p-2 rounded text-center">
                                        <div className="text-xs text-gray-500">Span Coverage</div>
                                        <div className={{`text-xl font-bold ${{
                                            (selectedEdge.alignment.span_coverage || 0) >= 0.7 ? 'text-green-600' :
                                            (selectedEdge.alignment.span_coverage || 0) >= 0.3 ? 'text-yellow-600' :
                                            'text-red-600'
                                        }}`}}>
                                            {{((selectedEdge.alignment.span_coverage || 0) * 100).toFixed(1)}}%
                                        </div>
                                        <div className="text-xs text-gray-400">reuse vs padding</div>
                                    </div>
                                    <div className="bg-white p-2 rounded text-center">
                                        <div className="text-xs text-gray-500">Content Weight</div>
                                        <div className={{`text-xl font-bold ${{
                                            (selectedEdge.alignment.content_weight || 0) >= 1.5 ? 'text-green-600' :
                                            (selectedEdge.alignment.content_weight || 0) >= 1.0 ? 'text-yellow-600' :
                                            'text-gray-600'
                                        }}`}}>
                                            {{(selectedEdge.alignment.content_weight || 0).toFixed(2)}}
                                        </div>
                                        <div className="text-xs text-gray-400">avg IDF</div>
                                    </div>
                                </div>
                                {{/* Raw counts */}}
                                <div className="flex gap-4 text-sm text-gray-600">
                                    <span>{{selectedEdge.alignment.length}} tokens</span>
                                    <span>{{selectedEdge.alignment.lemma_matches}} matches</span>
                                    <span>{{selectedEdge.alignment.substitutions || 0}} subs</span>
                                    <span>{{selectedEdge.alignment.gaps}} gaps</span>
                                </div>
                            </div>

                            {{/* Side-by-side passages */}}
                            <div className="grid grid-cols-2 gap-4">
                                <PassageDisplay
                                    title="Source"
                                    bookTitle={{data.book_a.title || `Book ${{data.book_a.id}}`}}
                                    location={{selectedEdge.source.location}}
                                    text={{selectedEdge.source.text}}
                                />
                                <PassageDisplay
                                    title="Target"
                                    bookTitle={{data.book_b.title || `Book ${{data.book_b.id}}`}}
                                    location={{selectedEdge.target.location}}
                                    text={{selectedEdge.target.text}}
                                />
                            </div>
                        </div>
                    ) : (
                        <div className="h-full flex items-center justify-center text-gray-500">
                            Select a match to view details
                        </div>
                    )}}
                </div>
            </div>
        </div>
    );
}}

// Passage Display Component
function PassageDisplay({{ title, bookTitle, location, text }}) {{
    return (
        <div className="p-4 border rounded-lg bg-white">
            <div className="mb-3">
                <h3 className="font-bold text-lg">{{title}}</h3>
                <p className="text-sm text-gray-600">{{bookTitle}}</p>
                <p className="text-sm text-gray-500">{{location}}</p>
            </div>
            <div className="arabic-text text-right leading-loose" dir="rtl" lang="ar">
                <span className="context-text">{{text.before}}</span>
                {{text.before && ' '}}
                <span className="highlight-match">{{text.matched}}</span>
                {{text.after && ' '}}
                <span className="context-text">{{text.after}}</span>
            </div>
        </div>
    );
}}

// Render the app
const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(<App />);
"##;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_edge() -> ReuseEdge {
        ReuseEdge {
            id: 1,
            source_book_id: 100,
            source_start_page: (1, 10),
            source_start_offset: 5,
            source_end_page: (1, 15),
            source_end_offset: 20,
            source_global_start: 500,
            source_global_end: 600,
            target_book_id: 200,
            target_start_page: (2, 5),
            target_start_offset: 10,
            target_end_page: (2, 10),
            target_end_offset: 30,
            target_global_start: 1000,
            target_global_end: 1100,
            aligned_length: 100,
            lemma_matches: 85,
            substitutions: 5,
            root_only_matches: 10,
            gaps: 5,
            core_similarity: 0.944,  // 85 / (85 + 5)
            span_coverage: 0.90,     // (85 + 5) / 100
            content_weight: 1.5,
            lemma_similarity: 0.85,
            combined_similarity: 0.90,
            weighted_similarity: 0.85,
            avg_match_weight: 1.5,
        }
    }

    #[test]
    fn test_format_page_location() {
        assert_eq!(format_page_location(1, 10, 5), "1:10.5");
        assert_eq!(format_page_location(0, 0, 0), "0:0.0");
    }

    #[test]
    fn test_format_edge() {
        let edge = create_test_edge();
        let formatted = format_edge(&edge);

        assert!(formatted.contains("Edge 1"));
        assert!(formatted.contains("Book 100"));
        assert!(formatted.contains("Book 200"));
        assert!(formatted.contains("len=100"));
        assert!(formatted.contains("matches=85"));
        assert!(formatted.contains("subs=5"));
        assert!(formatted.contains("Core: 94.4%"));
        assert!(formatted.contains("Coverage: 90.0%"));
        assert!(formatted.contains("Weight: 1.50"));
    }

    #[test]
    fn test_write_csv() {
        let edges = vec![create_test_edge()];
        let mut output = Vec::new();

        write_csv(&edges, &mut output).unwrap();

        let csv = String::from_utf8(output).unwrap();
        assert!(csv.contains("id,source_book_id")); // Header
        assert!(csv.contains("1,100,1,10")); // Data
    }

    #[test]
    fn test_write_csv_empty() {
        let edges: Vec<ReuseEdge> = vec![];
        let mut output = Vec::new();

        write_csv(&edges, &mut output).unwrap();

        let csv = String::from_utf8(output).unwrap();
        // Should only have header
        assert!(csv.contains("id,source_book_id"));
        assert_eq!(csv.lines().count(), 1);
    }
}
