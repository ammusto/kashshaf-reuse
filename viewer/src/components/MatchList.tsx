import type { ReuseEdge, Validations } from '../types';

interface Props {
  edges: ReuseEdge[];
  selectedId: number | undefined;
  validations: Validations;
  onSelect: (edge: ReuseEdge) => void;
}

function getSimilarityClass(similarity: number): string {
  if (similarity >= 0.9) return 'similarity-high';
  if (similarity >= 0.7) return 'similarity-medium';
  return 'similarity-low';
}

function getMetricColor(value: number, thresholds: { high: number; medium: number }): string {
  if (value >= thresholds.high) return 'text-green-600';
  if (value >= thresholds.medium) return 'text-yellow-600';
  return 'text-red-600';
}

export function MatchList({ edges, selectedId, validations, onSelect }: Props) {
  return (
    <div className="w-80 border-r overflow-auto bg-white custom-scrollbar">
      <div className="p-2 bg-gray-100 font-bold sticky top-0 z-10 border-b">
        {edges.length} matches
      </div>

      {edges.map(edge => (
        <div
          key={edge.id}
          onClick={() => onSelect(edge)}
          className={`p-3 border-b cursor-pointer hover:bg-gray-50 transition-colors ${
            selectedId === edge.id ? 'bg-blue-50 border-l-4 border-l-blue-500' : ''
          }`}
        >
          <div className="flex justify-between items-start">
            <span className="text-sm text-gray-500">#{edge.id}</span>
            <div className="flex items-center gap-1">
              {validations[edge.id] === 'valid' && (
                <span className="text-green-500">✓</span>
              )}
              {validations[edge.id] === 'noise' && (
                <span className="text-red-500">✗</span>
              )}
              <span className={`text-sm font-bold ${getSimilarityClass(edge.alignment.similarity)}`}>
                {(edge.alignment.similarity * 100).toFixed(0)}%
              </span>
            </div>
          </div>

          <div className="text-sm mt-1 text-gray-600 flex gap-3">
            <span>{edge.alignment.length} tok</span>
            <span className={`${getMetricColor(edge.alignment.core_similarity ?? edge.alignment.similarity, { high: 0.9, medium: 0.7 })}`}>
              C:{((edge.alignment.core_similarity ?? edge.alignment.similarity) * 100).toFixed(0)}%
            </span>
            <span className={`${getMetricColor(edge.alignment.span_coverage ?? 1, { high: 0.7, medium: 0.3 })}`}>
              S:{((edge.alignment.span_coverage ?? 1) * 100).toFixed(0)}%
            </span>
            <span className={`${getMetricColor(edge.alignment.content_weight ?? 1, { high: 1.5, medium: 1.0 })}`}>
              W:{(edge.alignment.content_weight ?? 1).toFixed(1)}
            </span>
          </div>

          {/* Preview of matched text */}
          <div
            className="text-sm text-gray-500 mt-1 truncate arabic-text"
            dir="rtl"
            lang="ar"
          >
            {edge.source.text.matched.slice(0, 60)}
            {edge.source.text.matched.length > 60 ? '...' : ''}
          </div>
        </div>
      ))}

      {edges.length === 0 && (
        <div className="p-4 text-center text-gray-500">
          No matches found with current filters
        </div>
      )}
    </div>
  );
}
