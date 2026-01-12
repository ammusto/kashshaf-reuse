import { useState, useMemo, useCallback } from 'react';
import type { ComparisonResult, Filters, Validations, ValidationStatus } from '../types';

export function useMatchData(data: ComparisonResult | null) {
  const [filters, setFilters] = useState<Filters>({
    minSimilarity: 0,
    minLength: 0,
    minCoreSimilarity: 0,
    minSpanCoverage: 0,
    minContentWeight: 0,
    searchText: '',
    sortBy: 'core_similarity',
    sortDesc: true,
  });

  const [validations, setValidations] = useState<Validations>({});

  const filteredEdges = useMemo(() => {
    if (!data) return [];

    let edges = data.edges.filter(edge => {
      // Legacy similarity filter
      if (edge.alignment.similarity < filters.minSimilarity) return false;
      if (edge.alignment.length < filters.minLength) return false;

      // Three-metric filters (with fallbacks for older data)
      const coreSim = edge.alignment.core_similarity ?? edge.alignment.similarity;
      const spanCov = edge.alignment.span_coverage ?? 1;
      const contentWt = edge.alignment.content_weight ?? edge.alignment.avg_match_weight ?? 1;

      if (coreSim < filters.minCoreSimilarity) return false;
      if (spanCov < filters.minSpanCoverage) return false;
      if (contentWt < filters.minContentWeight) return false;

      // Text search
      if (filters.searchText !== '' &&
          !edge.source.text.matched.includes(filters.searchText) &&
          !edge.target.text.matched.includes(filters.searchText)) {
        return false;
      }

      return true;
    });

    // Sort
    edges = [...edges].sort((a, b) => {
      let cmp = 0;
      switch (filters.sortBy) {
        case 'core_similarity':
          cmp = (a.alignment.core_similarity ?? a.alignment.similarity) -
                (b.alignment.core_similarity ?? b.alignment.similarity);
          break;
        case 'span_coverage':
          cmp = (a.alignment.span_coverage ?? 1) - (b.alignment.span_coverage ?? 1);
          break;
        case 'content_weight':
          cmp = (a.alignment.content_weight ?? 1) - (b.alignment.content_weight ?? 1);
          break;
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
      }
      return filters.sortDesc ? -cmp : cmp;
    });

    return edges;
  }, [data, filters]);

  const setValidation = useCallback((edgeId: number, status: ValidationStatus) => {
    setValidations(v => ({ ...v, [edgeId]: status }));
  }, []);

  const updateFilter = useCallback(<K extends keyof Filters>(key: K, value: Filters[K]) => {
    setFilters(f => ({ ...f, [key]: value }));
  }, []);

  const toggleSortOrder = useCallback(() => {
    setFilters(f => ({ ...f, sortDesc: !f.sortDesc }));
  }, []);

  const validCount = useMemo(() =>
    Object.values(validations).filter(v => v === 'valid').length,
    [validations]
  );

  const noiseCount = useMemo(() =>
    Object.values(validations).filter(v => v === 'noise').length,
    [validations]
  );

  const exportValidated = useCallback(() => {
    const validated = filteredEdges.filter(e => validations[e.id]);
    const csvContent = [
      ['id', 'validation', 'source_location', 'source_text', 'target_location', 'target_text',
       'length', 'lemma_matches', 'substitutions', 'gaps',
       'core_similarity', 'span_coverage', 'content_weight', 'similarity'].join(','),
      ...validated.map(e => [
        e.id,
        validations[e.id],
        `"${e.source.location}"`,
        `"${e.source.text.matched.replace(/"/g, '""')}"`,
        `"${e.target.location}"`,
        `"${e.target.text.matched.replace(/"/g, '""')}"`,
        e.alignment.length,
        e.alignment.lemma_matches,
        e.alignment.substitutions ?? 0,
        e.alignment.gaps,
        (e.alignment.core_similarity ?? e.alignment.similarity).toFixed(4),
        (e.alignment.span_coverage ?? 1).toFixed(4),
        (e.alignment.content_weight ?? 1).toFixed(4),
        e.alignment.similarity.toFixed(4)
      ].join(','))
    ].join('\n');

    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'validated_matches.csv';
    a.click();
    URL.revokeObjectURL(url);
  }, [filteredEdges, validations]);

  return {
    filteredEdges,
    filters,
    validations,
    validCount,
    noiseCount,
    setValidation,
    updateFilter,
    toggleSortOrder,
    exportValidated,
  };
}
