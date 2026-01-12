// Types matching the Rust ComparisonResultWithText JSON structure

export interface ComparisonResult {
  version: string;
  generated_at: string;
  parameters: ComparisonParams;
  book_a: BookInfo;
  book_b: BookInfo;
  summary: ComparisonSummary;
  edges: ReuseEdge[];
}

export interface ComparisonParams {
  window_size: number;
  stride: number;
  ngram_size: number;
  min_shared_shingles: number;
  min_length: number;
  min_similarity: number;
  match_score: number;
  mismatch_penalty: number;
  gap_penalty: number;
  brute_force: boolean;
}

export interface BookInfo {
  id: number;
  title: string;
  author: string;
  death_ah: number | null;
  token_count: number;
  page_count: number;
}

export interface ComparisonSummary {
  edge_count: number;
  total_aligned_tokens: number;
  book_a_coverage: number;
  book_b_coverage: number;
  avg_similarity: number;
}

export interface ReuseEdge {
  id: number;
  source: PassageRef;
  target: PassageRef;
  alignment: AlignmentInfo;
}

export interface PassageRef {
  book_id: number;
  location: string;
  global_range: [number, number];
  text: PassageText;
}

export interface PassageText {
  before: string;
  matched: string;
  after: string;
}

export interface AlignmentInfo {
  length: number;
  lemma_matches: number;
  substitutions: number;
  root_only_matches: number;
  gaps: number;
  // Three orthogonal metrics
  core_similarity: number;      // matches / (matches + subs) - quotation exactness
  span_coverage: number;        // (matches + subs) / aligned_length - reuse vs padding
  content_weight: number;       // avg IDF of matched lemmas
  // Lexical diversity: unique_matched_lemmas / lemma_matches
  // Low values (< 0.55) indicate formulaic content; high values indicate substantive reuse
  lexical_diversity: number;
  // Legacy metrics
  similarity: number;
  combined_similarity: number;
  weighted_similarity: number;
  avg_match_weight: number;
}

export interface Filters {
  minSimilarity: number;
  minLength: number;
  minCoreSimilarity: number;
  minSpanCoverage: number;
  minContentWeight: number;
  searchText: string;
  sortBy: 'similarity' | 'length' | 'position' | 'id' | 'core_similarity' | 'span_coverage' | 'content_weight';
  sortDesc: boolean;
}

export type ValidationStatus = 'valid' | 'noise' | undefined;

export interface Validations {
  [edgeId: number]: ValidationStatus;
}
