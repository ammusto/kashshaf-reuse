# Kashshaf Text Reuse Detection Pipeline

High-performance text reuse detection for premodern Arabic texts, written in Rust.

## Overview

Kashshaf-reuse detects text reuse (quotations, paraphrases, shared passages) between Arabic books by comparing **lemma ID sequences** rather than surface text. This approach automatically handles Arabic's rich morphological variation - words like كتاب, كتب, الكتاب, and وكتاب all share the same lemma_id and are treated as matches.

**New in v0.6:** Lexical Diversity metric. Suppresses formulaic reuse (e.g., isnād-style phrases) by measuring unique lemmas / total matches. Low diversity indicates repetitive content even when individual words have moderate IDF scores.

**v0.5:** Three-metric scoring system. Replaces single similarity % with three orthogonal metrics that match human scholarly judgment: Core Similarity (quotation exactness), Span Coverage (reuse vs padding), and Content Weight (average IDF).

**v0.4:** Frequency-weighted alignment scoring using document-internal IDF. Rare lemmas contribute more to alignment than common words, suppressing formulaic overlap and strengthening informative reuse detection.

**v0.3:** Root-based matching support. Match words that share the same Arabic root (جذر) even when lemmas differ, catching paraphrases where different derivations of the same root are used.

**v0.2:** Full Arabic text reconstruction and interactive HTML viewer for browsing and validating matches.

## Performance

| Metric | Result |
|--------|--------|
| Smith-Waterman alignment (275×275) | **36,900 pairs/sec** |
| N-gram filtering effectiveness | 95%+ reduction in comparisons |
| Full book pair comparison | Seconds to minutes depending on size |

## Installation

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- SQLite corpus database with tokenized Arabic texts

### Build

```bash
cd kashshaf-reuse
cargo build --release
```

The binary will be at `./target/release/kashshaf-reuse`.

## Usage

### Compare Two Books (with Arabic text)

```bash
# JSON output with reconstructed Arabic text (lemma matching, default)
./target/release/kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 \
    --book-b 553 \
    --output ./output/230_553.json \
    --csv \
    --context-tokens 30 \
    --show-edges 5

# Combined mode: lemma + root matching (recommended for paraphrase detection)
./target/release/kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 \
    --book-b 553 \
    --output ./output/230_553_combined.json \
    --mode combined \
    --lemma-score 2 \
    --root-score 1

# Root-only matching (experimental, may be noisy with common roots)
./target/release/kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 \
    --book-b 553 \
    --output ./output/230_553_root.json \
    --mode root

# Self-contained HTML viewer (opens in browser)
./target/release/kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 \
    --book-b 553 \
    --output ./output/230_553_viewer \
    --format viewer
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--corpus-db` | required | Path to corpus.db |
| `--book-a` | required | First book ID |
| `--book-b` | required | Second book ID |
| `--output` | required | Output file path |
| `--format` | json | Output format: `json`, `csv`, or `viewer` (HTML) |
| `--csv` | false | Also output CSV file |
| `--include-text` | true | Include reconstructed Arabic text |
| `--context-tokens` | 30 | Context tokens before/after each match |
| `--window-size` | 275 | Window size in tokens |
| `--stride` | 60 | Stride between windows |
| `--ngram-size` | 5 | N-gram size for filtering |
| `--min-shared-shingles` | 3 | Minimum shared shingles to compare |
| `--min-length` | 10 | Minimum aligned length |
| `--min-similarity` | 0.4 | Minimum similarity ratio (0.0-1.0) |
| `--mode` | lemma | Matching mode: `lemma`, `root`, or `combined` |
| `--lemma-score` | 2 | Score for lemma match (used in combined mode) |
| `--root-score` | 1 | Score for root-only match (same root, different lemma) |
| `--use-weights` | true | Enable document-internal IDF weighting |
| `--min-weighted-similarity` | none | Filter by IDF-weighted similarity |
| `--min-core-similarity` | 0.85 | Filter by core similarity (quotation exactness) |
| `--min-span-coverage` | 0.30 | Filter by span coverage (reuse vs padding) |
| `--min-content-weight` | 1.10 | Filter by content weight (avg lemma IDF) |
| `--min-lexical-diversity` | 0.55 | Filter by lexical diversity (unique lemmas / matches) |
| `--no-filters` | false | Disable all metric filters (exploratory mode) |
| `--brute-force` | false | Skip filtering, compare all pairs |
| `--quiet` | false | Suppress progress output |
| `--show-edges` | none | Print first N edges to console |

### Matching Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `lemma` | Only count exact lemma matches (default) | Precise quotation detection |
| `root` | Only count root matches (ignoring lemma) | Experimental, may be noisy |
| `combined` | Lemma match = full score, root-only = partial | Paraphrase detection, best recall |

**Combined mode** is recommended when you want to catch both exact quotations and paraphrases. It scores lemma matches at full value (default: 2) and root-only matches at partial value (default: 1). This catches cases where an author uses a different derivation of the same root (e.g., كاتب vs مكتوب - both from root ك-ت-ب).

### IDF Weighting (v0.4+)

By default, alignment scoring uses **document-internal IDF weighting** to prioritize rare vocabulary over common words:

```
weight(lemma) = ln(total_tokens / document_frequency)  # clamped to [0.5, 3.0]
```

| Word Type | Example | Weight | Effect |
|-----------|---------|--------|--------|
| Very common | من، في، على | ~0.5 | Weak signal |
| Common | قال، كان، هذا | ~0.7-1.0 | Normal signal |
| Uncommon | حديث، رواية | ~1.2-1.8 | Strong signal |
| Rare/technical | قشع، تقعقع | ~2.5-3.0 | Very strong signal |

**Benefits:**
- Formulaic overlaps (isnād chains, common phrases) naturally weaken
- Technical vocabulary anchors alignments more strongly
- Results sort cleanly by informational value using `weighted_similarity`
- No manual stopword lists needed

**IDF Metrics:**
| Metric | Meaning |
|--------|---------|
| `weighted_similarity` | IDF-weighted match density (informational value) |
| `avg_match_weight` | Average rarity of matched vocabulary (diagnostic) |

To disable IDF weighting and use unweighted scoring:

### Three-Metric Scoring System (v0.5+)

Traditional similarity metrics (like `lemma_matches / aligned_length`) are confounded by gaps - an exact quote embedded in commentary shows low similarity because gaps from commentary dilute the score. The three-metric system separates orthogonal aspects of reuse:

#### Core Similarity
```
core_similarity = lemma_matches / (lemma_matches + substitutions)
```
- Measures **quotation exactness** ignoring gaps
- High = verbatim quotation, Low = loose paraphrase
- **0.9+** = verbatim, **0.7-0.9** = light paraphrase, **<0.7** = loose
- If `lemma_matches == 0`, then `core_similarity = 0`

#### Span Coverage
```
span_coverage = (lemma_matches + substitutions) / aligned_length
```
- Measures **how much of the alignment is actual content vs gaps**
- Low coverage = quote is embedded in commentary/gloss
- **0.7+** = standalone quote, **0.3-0.7** = embedded, **<0.3** = mostly scaffolding

#### Content Weight
```
content_weight = match_weight_sum / lemma_matches
```
- **Average IDF** of matched lemmas
- High = substantive/technical vocabulary, Low = formulaic content
- If `lemma_matches == 0`, then `content_weight = 0`

#### Lexical Diversity (v0.6+)
```
lexical_diversity = unique_matched_lemmas / lemma_matches
```
- Measures **vocabulary variety** within the alignment
- Low diversity = same lemmas repeat (formulaic content)
- **0.7+** = substantive, **0.55-0.7** = moderate, **<0.55** = formulaic
- Complements IDF: IDF weights rare words across the document, diversity detects repetition within the match

**Why lexical diversity helps:**

IDF weighting alone doesn't catch all formulaic content. Consider isnād phrases like "حدثنا فلان عن فلان عن فلان" - the words may have moderate IDF scores individually, but the same lemmas repeat multiple times in the alignment. Lexical diversity catches this pattern:

| Scenario | IDF Weight | Lexical Diversity | Verdict |
|----------|------------|-------------------|---------|
| Technical quote | High | High | Substantive |
| Varied narrative | Medium | High | Substantive |
| Repetitive isnād | Medium | Low | Formulaic |
| Common phrases | Low | Low | Formulaic |

#### Root-Only Matches Note

Root-only matches influence alignment discovery and `combined_similarity` but are **excluded from the three quotation-exactness metrics**. This is intentional: core similarity measures exact quotation fidelity, not paraphrase.

#### Interpretation Table

| Scenario | Core | Coverage | Weight | Diversity |
|----------|------|----------|--------|-----------|
| Exact standalone quote | High | High | Medium+ | High |
| Quote embedded in gloss | High | Low | Medium+ | High |
| Formulaic overlap (isnād) | High | High | Low | Low |
| Repetitive chains | High | High | Medium | Low |
| Paraphrase | Medium | Medium | Medium | High |
| Noise | Low | Low | Low | Varies |

#### Recommended Modern Usage

While `--min-similarity` is retained for backward compatibility, the recommended approach for new workflows is to use the metric filters (all enabled by default):

```bash
# Default filtering (recommended - all metrics enabled)
./kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 --book-b 553 \
    --output ./output/high_quality.json

# Customize thresholds
./kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 --book-b 553 \
    --min-core-similarity 0.90 \
    --min-lexical-diversity 0.60 \
    --output ./output/strict.json

# Exploratory mode (disable all filters)
./kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 --book-b 553 \
    --no-filters \
    --output ./output/exploratory.json

# Legacy mode (backward compatible)
./kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 --book-b 553 \
    --no-filters \
    --min-similarity 0.4 \
    --output ./output/legacy.json
```

#### Console Output Format
```
Edge 145: len=32 matches=12 subs=2 gaps=18
  Core: 85.7%  Coverage: 43.8%  Weight: 1.82  Diversity: 0.75 (substantive)
  Book 230 [1:15.42→1:16.18] ↔ Book 553 [1:3.105→1:4.22]
```
```bash
./kashshaf-reuse compare --use-weights false ...
```

### Show Corpus Statistics

```bash
./target/release/kashshaf-reuse stats --corpus-db ./data/corpus.db
```

### Show Book Information

```bash
./target/release/kashshaf-reuse info --corpus-db ./data/corpus.db --book-id 230 --show-pages
```

### Run Performance Benchmark

```bash
./target/release/kashshaf-reuse benchmark --iterations 10000 --size 275
```

## Output Formats

### JSON with Text (default)

When `--include-text` is enabled (default), the output includes reconstructed Arabic text:

```json
{
  "version": "0.1.0",
  "generated_at": "2025-01-11T12:00:00Z",
  "parameters": { ... },
  "book_a": {
    "id": 230,
    "title": "غريب الحديث",
    "author": "أبو عبيد القاسم بن سلام",
    "death_ah": 224,
    "token_count": 153198,
    "page_count": 512
  },
  "book_b": { ... },
  "summary": {
    "edge_count": 145,
    "total_aligned_tokens": 8234,
    "book_a_coverage": 0.054,
    "book_b_coverage": 0.531,
    "avg_similarity": 0.72,
    "avg_weighted_similarity": 0.85
  },
  "edges": [
    {
      "id": 1,
      "source": {
        "book_id": 230,
        "location": "1:15.42 → 1:16.18",
        "global_range": [4521, 4612],
        "text": {
          "before": "وهذا من كلام العرب في الجاهلية",
          "matched": "قال أبو عبيد في حديث النبي صلى الله عليه وسلم",
          "after": "وقال غيره في هذا المعنى"
        }
      },
      "target": {
        "book_id": 553,
        "location": "1:3.105 → 1:4.22",
        "global_range": [892, 983],
        "text": {
          "before": "قال ابن قتيبة وأما قول أبي عبيد",
          "matched": "قال أبو عبيد في حديث النبي صلى الله عليه وسلم",
          "after": "فهذا غلط منه والصواب"
        }
      },
      "alignment": {
        "length": 91,
        "lemma_matches": 78,
        "substitutions": 3,
        "root_only_matches": 8,
        "gaps": 5,
        "core_similarity": 0.963,
        "span_coverage": 0.890,
        "content_weight": 1.08,
        "lexical_diversity": 0.74,
        "similarity": 0.857,
        "combined_similarity": 0.901,
        "weighted_similarity": 0.92,
        "avg_match_weight": 1.08
      }
    }
  ]
}
```

### HTML Viewer

Use `--format viewer` to generate a self-contained HTML file with an interactive React-based viewer:

```bash
./target/release/kashshaf-reuse compare \
    --corpus-db ./data/corpus.db \
    --book-a 230 --book-b 553 \
    --output ./output/comparison \
    --format viewer
```

The viewer includes:
- **Match list** with similarity color-coding (green/yellow/red)
- **Side-by-side passage display** with highlighted matches and context
- **Filtering** by similarity, length, and Arabic text search
- **Sorting** by similarity, length, position, or ID
- **Validation** buttons to mark matches as valid or noise
- **Export** validated matches to CSV

The HTML file works offline in any modern browser - no server required.

### CSV Output

Use `--csv` to also output a CSV file with all match data including Arabic text.

## Algorithm

### Pipeline Overview

1. **Load** token-to-lemma mapping from `token_definitions` table
2. **Extract** lemma streams for both books from `page_tokens` table
3. **Generate** overlapping windows (default: 275 tokens, stride 60)
4. **Filter** candidate pairs using n-gram shingles (5-grams by default)
5. **Align** candidate pairs using Smith-Waterman local alignment
6. **Merge** overlapping edges into maximal spans
7. **Output** results as JSON/CSV

### Smith-Waterman Alignment

The core algorithm uses Smith-Waterman local alignment on lemma ID sequences:

**Lemma mode (default):**
- **Match score**: +2 (when lemma IDs are equal)
- **Mismatch penalty**: -1
- **Gap penalty**: -1

**Combined mode:**
- **Lemma match score**: configurable (default: 2)
- **Root-only match score**: configurable (default: 1) - same root but different lemma
- **Mismatch penalty**: -1
- **Gap penalty**: -1

This finds the best local alignment between two windows, allowing for insertions, deletions, and substitutions. Combined mode is recommended for paraphrase detection as it catches cases where authors use different derivations of the same Arabic root.

### N-gram Filtering

Before expensive alignment, windows are filtered using n-gram shingles:

1. Generate all 5-grams (consecutive lemma ID sequences) for each window
2. Build an inverted index of shingles for book B
3. Only compare window pairs that share at least 3 shingles

This typically eliminates 95%+ of comparisons.

## Database Schema

The tool expects a SQLite database with these tables:

```sql
-- Token definitions with lemma mappings
CREATE TABLE token_definitions (
    id INTEGER PRIMARY KEY,
    surface TEXT NOT NULL,
    lemma_id INTEGER NOT NULL,
    root_id INTEGER,
    pos_id INTEGER NOT NULL,
    feature_set_id INTEGER NOT NULL,
    clitic_set_id INTEGER NOT NULL
);

-- Page token arrays (little-endian u32 blob)
CREATE TABLE page_tokens (
    book_id INTEGER NOT NULL,
    part_index INTEGER NOT NULL,
    page_id INTEGER NOT NULL,
    token_ids BLOB NOT NULL,
    PRIMARY KEY (book_id, part_index, page_id)
);

-- Lemma lookup
CREATE TABLE lemmas (
    id INTEGER PRIMARY KEY,
    lemma TEXT UNIQUE NOT NULL
);
```

## Library Usage

The crate can also be used as a library:

```rust
use kashshaf_reuse::prelude::*;
use std::path::Path;

let db_path = Path::new("corpus.db");
let params = ComparisonParams::default();

// Basic comparison (lemma IDs only)
let token_to_lemma = load_token_to_lemma(db_path)?;
let stream_a = load_book_lemma_stream(db_path, 230, &token_to_lemma)?;
let stream_b = load_book_lemma_stream(db_path, 553, &token_to_lemma)?;
let result = compare_books_from_streams(&stream_a, &stream_b, &params, true)?;
println!("Found {} reuse edges", result.edges.len());

// Comparison with text reconstruction
let context_tokens = 30;
let result_with_text = compare_books_with_text(
    230, 553, db_path, &params, context_tokens, true
)?;

for edge in &result_with_text.edges {
    println!("Source: {}", edge.source.text.matched);
    println!("Target: {}", edge.target.text.matched);
    println!("Similarity: {:.1}%", edge.alignment.similarity * 100.0);
}
```

## React Viewer Development

A standalone React viewer is included in the `viewer/` directory for development:

```bash
cd viewer
npm install
npm run dev
```

The viewer uses Vite + React + TypeScript + Tailwind CSS. Place a `comparison_result.json` file in the viewer directory to load real data, or it will use sample data for development.

## License

MIT
