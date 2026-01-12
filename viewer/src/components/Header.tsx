import type { BookInfo, ComparisonSummary } from '../types';

interface Props {
  bookA: BookInfo;
  bookB: BookInfo;
  summary: ComparisonSummary;
  onOpenFile?: () => void;
}

export function Header({ bookA, bookB, summary, onOpenFile }: Props) {
  return (
    <header className="bg-white border-b px-4 py-3">
      <div className="flex justify-between items-center flex-wrap gap-2">
        <div className="flex items-center gap-4">
          <div>
            <h1 className="text-xl font-bold">Kashshaf Text Reuse Viewer</h1>
            <p className="text-sm text-gray-600">
              {bookA.title || `Book ${bookA.id}`} vs {bookB.title || `Book ${bookB.id}`}
            </p>
          </div>
          {onOpenFile && (
            <button
              onClick={onOpenFile}
              className="text-sm px-3 py-1 border rounded hover:bg-gray-50 transition-colors"
              title="Open another file"
            >
              Open File
            </button>
          )}
        </div>
        <div className="text-right text-sm">
          <div>{summary.edge_count} total matches</div>
          <div className="text-gray-500">
            {bookA.token_count.toLocaleString()} + {bookB.token_count.toLocaleString()} tokens
          </div>
        </div>
      </div>
    </header>
  );
}
