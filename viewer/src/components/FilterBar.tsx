import type { Filters } from '../types';

interface Props {
  filters: Filters;
  onUpdateFilter: <K extends keyof Filters>(key: K, value: Filters[K]) => void;
  onToggleSortOrder: () => void;
  onExport: () => void;
}

export function FilterBar({ filters, onUpdateFilter, onToggleSortOrder, onExport }: Props) {
  return (
    <div className="bg-white px-4 py-2 border-b flex gap-4 items-center text-sm flex-wrap">
      <label className="flex items-center gap-2">
        <span className="whitespace-nowrap">Core:</span>
        <input
          type="range"
          min="0"
          max="100"
          value={(filters.minCoreSimilarity ?? 0) * 100}
          onChange={e => onUpdateFilter('minCoreSimilarity', parseInt(e.target.value) / 100)}
          className="w-20"
        />
        <span className="w-10">{((filters.minCoreSimilarity ?? 0) * 100).toFixed(0)}%</span>
      </label>

      <label className="flex items-center gap-2">
        <span className="whitespace-nowrap">Coverage:</span>
        <input
          type="range"
          min="0"
          max="100"
          value={(filters.minSpanCoverage ?? 0) * 100}
          onChange={e => onUpdateFilter('minSpanCoverage', parseInt(e.target.value) / 100)}
          className="w-20"
        />
        <span className="w-10">{((filters.minSpanCoverage ?? 0) * 100).toFixed(0)}%</span>
      </label>

      <label className="flex items-center gap-2">
        <span className="whitespace-nowrap">Weight:</span>
        <input
          type="range"
          min="0"
          max="30"
          value={(filters.minContentWeight ?? 0) * 10}
          onChange={e => onUpdateFilter('minContentWeight', parseInt(e.target.value) / 10)}
          className="w-20"
        />
        <span className="w-10">{(filters.minContentWeight ?? 0).toFixed(1)}</span>
      </label>

      <label className="flex items-center gap-2">
        <span className="whitespace-nowrap">Length:</span>
        <input
          type="number"
          min="0"
          value={filters.minLength}
          onChange={e => onUpdateFilter('minLength', parseInt(e.target.value) || 0)}
          className="w-16 border rounded px-2 py-1"
        />
      </label>

      <label className="flex items-center gap-2">
        <span className="whitespace-nowrap">Search:</span>
        <input
          type="text"
          value={filters.searchText}
          onChange={e => onUpdateFilter('searchText', e.target.value)}
          placeholder="Arabic text..."
          className="w-48 border rounded px-2 py-1"
          dir="rtl"
        />
      </label>

      <label className="flex items-center gap-2">
        <span className="whitespace-nowrap">Sort by:</span>
        <select
          value={filters.sortBy}
          onChange={e => onUpdateFilter('sortBy', e.target.value as Filters['sortBy'])}
          className="border rounded px-2 py-1"
        >
          <option value="core_similarity">Core Sim</option>
          <option value="span_coverage">Coverage</option>
          <option value="content_weight">Weight</option>
          <option value="similarity">Legacy Sim</option>
          <option value="length">Length</option>
          <option value="position">Position</option>
          <option value="id">ID</option>
        </select>
      </label>

      <button
        onClick={onToggleSortOrder}
        className="border rounded px-2 py-1 hover:bg-gray-100"
      >
        {filters.sortDesc ? '↓ Desc' : '↑ Asc'}
      </button>

      <button
        onClick={onExport}
        className="ml-auto border rounded px-3 py-1 bg-blue-50 hover:bg-blue-100 text-blue-700"
      >
        Export Validated
      </button>
    </div>
  );
}
