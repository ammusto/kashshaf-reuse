import { useState, useEffect, useCallback, useRef } from 'react';
import type { ComparisonResult, ReuseEdge } from './types';
import { useMatchData } from './hooks/useMatchData';
import { Header, StatsBar, FilterBar, MatchList, DetailView } from './components';

declare global {
  interface Window {
    __COMPARISON_DATA__?: ComparisonResult;
  }
}

function App() {
  const [data, setData] = useState<ComparisonResult | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<ReuseEdge | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Check for embedded data first (from Rust CLI viewer output)
    if (window.__COMPARISON_DATA__) {
      setData(window.__COMPARISON_DATA__);
      setLoading(false);
      return;
    }

    // Try to load from comparison_result.json in the same directory
    fetch('./comparison_result.json')
      .then(res => {
        if (!res.ok) throw new Error('Could not load comparison_result.json');
        return res.json();
      })
      .then(jsonData => {
        setData(jsonData);
        setLoading(false);
      })
      .catch(() => {
        // No auto-loaded data, show file picker
        setLoading(false);
      });
  }, []);

  const handleFileSelect = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    setLoading(true);
    setError(null);

    const reader = new FileReader();
    reader.onload = (e) => {
      try {
        const jsonData = JSON.parse(e.target?.result as string) as ComparisonResult;
        if (!jsonData.edges || !jsonData.book_a || !jsonData.book_b) {
          throw new Error('Invalid comparison result format');
        }
        setData(jsonData);
        setLoading(false);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to parse JSON file');
        setLoading(false);
      }
    };
    reader.onerror = () => {
      setError('Failed to read file');
      setLoading(false);
    };
    reader.readAsText(file);
  }, []);

  const handleDrop = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    const file = event.dataTransfer.files[0];
    if (file && file.name.endsWith('.json')) {
      const input = fileInputRef.current;
      if (input) {
        const dataTransfer = new DataTransfer();
        dataTransfer.items.add(file);
        input.files = dataTransfer.files;
        input.dispatchEvent(new Event('change', { bubbles: true }));
      }
    }
  }, []);

  const handleDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
  }, []);

  const {
    filteredEdges,
    filters,
    validations,
    validCount,
    noiseCount,
    setValidation,
    updateFilter,
    toggleSortOrder,
    exportValidated,
  } = useMatchData(data);

  const handleSelectEdge = useCallback((edge: ReuseEdge) => {
    setSelectedEdge(edge);
  }, []);

  const handleValidate = useCallback((status: 'valid' | 'noise') => {
    if (selectedEdge) {
      setValidation(selectedEdge.id, status);
    }
  }, [selectedEdge, setValidation]);

  const handleOpenFile = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  if (loading) {
    return (
      <div className="h-screen flex items-center justify-center bg-gray-50">
        <div className="text-gray-500">Loading comparison data...</div>
      </div>
    );
  }

  if (!data) {
    return (
      <div
        className="h-screen flex items-center justify-center bg-gray-50"
        onDrop={handleDrop}
        onDragOver={handleDragOver}
      >
        <div className="text-center max-w-md p-8">
          <h1 className="text-2xl font-bold mb-4">Kashshaf Text Reuse Viewer</h1>

          {error && (
            <div className="text-red-500 mb-4 p-3 bg-red-50 rounded">{error}</div>
          )}

          <div className="border-2 border-dashed border-gray-300 rounded-lg p-8 mb-4 hover:border-blue-400 transition-colors">
            <input
              ref={fileInputRef}
              type="file"
              accept=".json"
              onChange={handleFileSelect}
              className="hidden"
              id="file-input"
            />
            <label
              htmlFor="file-input"
              className="cursor-pointer"
            >
              <div className="text-4xl mb-4">📂</div>
              <div className="text-lg font-medium mb-2">Open Comparison Result</div>
              <div className="text-sm text-gray-500 mb-4">
                Click to browse or drag & drop a JSON file
              </div>
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
              >
                Choose File
              </button>
            </label>
          </div>

          <div className="text-xs text-gray-400">
            Generate comparison results with:<br/>
            <code className="bg-gray-100 px-2 py-1 rounded">
              kashshaf-reuse compare --book-a ID --book-b ID --output result.json
            </code>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col bg-gray-50">
      <input
        ref={fileInputRef}
        type="file"
        accept=".json"
        onChange={handleFileSelect}
        className="hidden"
      />
      <Header
        bookA={data.book_a}
        bookB={data.book_b}
        summary={data.summary}
        onOpenFile={handleOpenFile}
      />
      <StatsBar
        summary={data.summary}
        filteredCount={filteredEdges.length}
        validCount={validCount}
        noiseCount={noiseCount}
      />
      <FilterBar
        filters={filters}
        onUpdateFilter={updateFilter}
        onToggleSortOrder={toggleSortOrder}
        onExport={exportValidated}
      />

      <div className="flex-1 flex overflow-hidden">
        <MatchList
          edges={filteredEdges}
          selectedId={selectedEdge?.id}
          validations={validations}
          onSelect={handleSelectEdge}
        />
        <DetailView
          edge={selectedEdge}
          bookA={data.book_a}
          bookB={data.book_b}
          validation={selectedEdge ? validations[selectedEdge.id] : undefined}
          onValidate={handleValidate}
        />
      </div>
    </div>
  );
}

export default App;
