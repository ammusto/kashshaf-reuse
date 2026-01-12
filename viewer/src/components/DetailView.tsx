import type { ReuseEdge, BookInfo, ValidationStatus } from '../types';
import { PassageDisplay } from './PassageDisplay';

interface Props {
  edge: ReuseEdge | null;
  bookA: BookInfo;
  bookB: BookInfo;
  validation: ValidationStatus;
  onValidate: (status: 'valid' | 'noise') => void;
}

function getMetricClass(value: number, thresholds: { high: number; medium: number }): string {
  if (value >= thresholds.high) return 'text-green-600';
  if (value >= thresholds.medium) return 'text-yellow-600';
  return 'text-red-600';
}

export function DetailView({ edge, bookA, bookB, validation, onValidate }: Props) {
  if (!edge) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-500">
        Select a match to view details
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col p-4 overflow-auto">
      {/* Header with stats */}
      <div className="mb-4 p-3 bg-gray-100 rounded-lg">
        {/* Top row: Basic info and validation */}
        <div className="flex justify-between items-center flex-wrap gap-2 mb-3">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-bold">Match #{edge.id}</span>
            <span className="text-gray-400">•</span>
            <span>{edge.alignment.length} tokens</span>
            <span className="text-gray-400">•</span>
            <span>{edge.alignment.lemma_matches} matches</span>
            <span className="text-gray-400">•</span>
            <span>{edge.alignment.substitutions ?? 0} subs</span>
            <span className="text-gray-400">•</span>
            <span>{edge.alignment.gaps} gaps</span>
          </div>

          {/* Validation buttons */}
          <div className="flex gap-2">
            <button
              onClick={() => onValidate('valid')}
              className={`px-3 py-1 rounded transition-colors ${
                validation === 'valid'
                  ? 'bg-green-500 text-white'
                  : 'bg-gray-200 hover:bg-green-100'
              }`}
            >
              ✓ Valid
            </button>
            <button
              onClick={() => onValidate('noise')}
              className={`px-3 py-1 rounded transition-colors ${
                validation === 'noise'
                  ? 'bg-red-500 text-white'
                  : 'bg-gray-200 hover:bg-red-100'
              }`}
            >
              ✗ Noise
            </button>
          </div>
        </div>

        {/* Three metrics row */}
        <div className="flex gap-6 pt-2 border-t border-gray-200">
          <div className="flex flex-col items-center">
            <span className="text-xs text-gray-500 uppercase tracking-wide">Core</span>
            <span className={`text-lg font-bold ${getMetricClass(edge.alignment.core_similarity ?? 0, { high: 0.9, medium: 0.7 })}`}>
              {((edge.alignment.core_similarity ?? edge.alignment.similarity) * 100).toFixed(1)}%
            </span>
            <span className="text-xs text-gray-400">exactness</span>
          </div>
          <div className="flex flex-col items-center">
            <span className="text-xs text-gray-500 uppercase tracking-wide">Coverage</span>
            <span className={`text-lg font-bold ${getMetricClass(edge.alignment.span_coverage ?? 0, { high: 0.7, medium: 0.3 })}`}>
              {((edge.alignment.span_coverage ?? 1) * 100).toFixed(1)}%
            </span>
            <span className="text-xs text-gray-400">reuse ratio</span>
          </div>
          <div className="flex flex-col items-center">
            <span className="text-xs text-gray-500 uppercase tracking-wide">Weight</span>
            <span className={`text-lg font-bold ${getMetricClass(edge.alignment.content_weight ?? 1, { high: 1.5, medium: 1.0 })}`}>
              {(edge.alignment.content_weight ?? edge.alignment.avg_match_weight ?? 1).toFixed(2)}
            </span>
            <span className="text-xs text-gray-400">avg IDF</span>
          </div>
        </div>
      </div>

      {/* Side-by-side passages */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <PassageDisplay
          title="Source"
          bookTitle={bookA.title || `Book ${bookA.id}`}
          location={edge.source.location}
          text={edge.source.text}
        />
        <PassageDisplay
          title="Target"
          bookTitle={bookB.title || `Book ${bookB.id}`}
          location={edge.target.location}
          text={edge.target.text}
        />
      </div>
    </div>
  );
}
