import type { ComparisonSummary } from '../types';

interface Props {
  summary: ComparisonSummary;
  filteredCount: number;
  validCount: number;
  noiseCount: number;
}

export function StatsBar({ summary, filteredCount, validCount, noiseCount }: Props) {
  return (
    <div className="bg-gray-100 px-4 py-2 border-b flex gap-6 text-sm flex-wrap">
      <span>
        Showing: <strong>{filteredCount}</strong> of {summary.edge_count} matches
      </span>
      <span className="text-green-600">✓ Valid: {validCount}</span>
      <span className="text-red-600">✗ Noise: {noiseCount}</span>
      <span className="text-gray-500">
        Book A coverage: {(summary.book_a_coverage * 100).toFixed(1)}% |
        Book B coverage: {(summary.book_b_coverage * 100).toFixed(1)}%
      </span>
      <span className="text-gray-500">
        Avg similarity: {(summary.avg_similarity * 100).toFixed(1)}%
      </span>
    </div>
  );
}
