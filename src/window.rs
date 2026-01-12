//! Windowing logic for generating overlapping windows from lemma streams.

use crate::models::{BookLemmaStream, BookTokenStream, ComparisonParams, Window};

/// Generate overlapping windows from a book's lemma stream.
///
/// Windows are created with the specified size and stride.
/// Each window contains a slice of lemma IDs and tracks its position
/// in both the flat lemma stream and the original page structure.
///
/// Note: This creates windows with empty root_ids for backward compatibility.
/// Use `generate_windows_with_roots` for root-based matching.
pub fn generate_windows(stream: &BookLemmaStream, params: &ComparisonParams) -> Vec<Window> {
    let flat_lemmas = stream.flat_lemmas();
    let mut windows = Vec::new();

    if flat_lemmas.is_empty() {
        return windows;
    }

    // Build page offset index for efficient position lookups
    let page_offsets = build_page_offsets(stream);

    if flat_lemmas.len() < params.window_size {
        // Book too small - single window containing all lemmas
        let (start_page, start_offset) = find_page_and_offset(&page_offsets, stream, 0);
        let (end_page, end_offset) =
            find_page_and_offset(&page_offsets, stream, flat_lemmas.len().saturating_sub(1));

        windows.push(Window {
            book_id: stream.book_id,
            window_idx: 0,
            global_start: 0,
            global_end: flat_lemmas.len(),
            start_page,
            start_offset,
            end_page,
            end_offset,
            lemma_ids: flat_lemmas.clone(),
            root_ids: vec![0; flat_lemmas.len()],  // Empty roots
        });
        return windows;
    }

    let mut window_idx = 0u32;
    let mut start = 0usize;

    while start + params.window_size <= flat_lemmas.len() {
        let end = start + params.window_size;

        let (start_page, start_offset) = find_page_and_offset(&page_offsets, stream, start);
        let (end_page, end_offset) = find_page_and_offset(&page_offsets, stream, end - 1);

        windows.push(Window {
            book_id: stream.book_id,
            window_idx,
            global_start: start,
            global_end: end,
            start_page,
            start_offset,
            end_page,
            end_offset,
            lemma_ids: flat_lemmas[start..end].to_vec(),
            root_ids: vec![0; end - start],  // Empty roots
        });

        window_idx += 1;
        start += params.stride;
    }

    // Handle final partial window if needed
    if start < flat_lemmas.len() && flat_lemmas.len() - start >= params.min_length {
        let (start_page, start_offset) = find_page_and_offset(&page_offsets, stream, start);
        let (end_page, end_offset) =
            find_page_and_offset(&page_offsets, stream, flat_lemmas.len() - 1);

        let remaining = flat_lemmas.len() - start;
        windows.push(Window {
            book_id: stream.book_id,
            window_idx,
            global_start: start,
            global_end: flat_lemmas.len(),
            start_page,
            start_offset,
            end_page,
            end_offset,
            lemma_ids: flat_lemmas[start..].to_vec(),
            root_ids: vec![0; remaining],  // Empty roots
        });
    }

    windows
}

/// Generate overlapping windows from a book's token stream with root support.
///
/// Windows are created with the specified size and stride.
/// Each window contains slices of lemma IDs and root IDs.
pub fn generate_windows_with_roots(stream: &BookTokenStream, params: &ComparisonParams) -> Vec<Window> {
    let flat_lemmas = stream.flat_lemma_ids();
    let flat_roots = stream.flat_root_ids();
    let mut windows = Vec::new();

    if flat_lemmas.is_empty() {
        return windows;
    }

    // Build page offset index for efficient position lookups
    let page_offsets = build_page_offsets_from_tokens(stream);

    if flat_lemmas.len() < params.window_size {
        // Book too small - single window containing all lemmas
        let (start_page, start_offset) = find_page_and_offset_tokens(&page_offsets, stream, 0);
        let (end_page, end_offset) =
            find_page_and_offset_tokens(&page_offsets, stream, flat_lemmas.len().saturating_sub(1));

        windows.push(Window {
            book_id: stream.book_id,
            window_idx: 0,
            global_start: 0,
            global_end: flat_lemmas.len(),
            start_page,
            start_offset,
            end_page,
            end_offset,
            lemma_ids: flat_lemmas,
            root_ids: flat_roots,
        });
        return windows;
    }

    let mut window_idx = 0u32;
    let mut start = 0usize;

    while start + params.window_size <= flat_lemmas.len() {
        let end = start + params.window_size;

        let (start_page, start_offset) = find_page_and_offset_tokens(&page_offsets, stream, start);
        let (end_page, end_offset) = find_page_and_offset_tokens(&page_offsets, stream, end - 1);

        windows.push(Window {
            book_id: stream.book_id,
            window_idx,
            global_start: start,
            global_end: end,
            start_page,
            start_offset,
            end_page,
            end_offset,
            lemma_ids: flat_lemmas[start..end].to_vec(),
            root_ids: flat_roots[start..end].to_vec(),
        });

        window_idx += 1;
        start += params.stride;
    }

    // Handle final partial window if needed
    if start < flat_lemmas.len() && flat_lemmas.len() - start >= params.min_length {
        let (start_page, start_offset) = find_page_and_offset_tokens(&page_offsets, stream, start);
        let (end_page, end_offset) =
            find_page_and_offset_tokens(&page_offsets, stream, flat_lemmas.len() - 1);

        windows.push(Window {
            book_id: stream.book_id,
            window_idx,
            global_start: start,
            global_end: flat_lemmas.len(),
            start_page,
            start_offset,
            end_page,
            end_offset,
            lemma_ids: flat_lemmas[start..].to_vec(),
            root_ids: flat_roots[start..].to_vec(),
        });
    }

    windows
}

/// Page offset entry for efficient position lookups
struct PageOffset {
    part_index: u32,
    page_id: u32,
    start_offset: usize,
    end_offset: usize, // exclusive
}

/// Build an index of page start/end offsets for efficient position lookups
fn build_page_offsets(stream: &BookLemmaStream) -> Vec<PageOffset> {
    let mut offsets = Vec::with_capacity(stream.pages.len());
    let mut current_offset = 0usize;

    for page in &stream.pages {
        let end_offset = current_offset + page.lemma_ids.len();
        offsets.push(PageOffset {
            part_index: page.part_index,
            page_id: page.page_id,
            start_offset: current_offset,
            end_offset,
        });
        current_offset = end_offset;
    }

    offsets
}

/// Find the page and offset within that page for a given global position.
/// Uses binary search for efficiency with large books.
fn find_page_and_offset(
    page_offsets: &[PageOffset],
    _stream: &BookLemmaStream,
    pos: usize,
) -> ((u32, u32), u32) {
    // Binary search to find the page containing this position
    let page_idx = page_offsets
        .binary_search_by(|offset| {
            if pos < offset.start_offset {
                std::cmp::Ordering::Greater
            } else if pos >= offset.end_offset {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .unwrap_or_else(|idx| idx.saturating_sub(1).min(page_offsets.len() - 1));

    let offset = &page_offsets[page_idx];
    let offset_within_page = (pos - offset.start_offset) as u32;

    ((offset.part_index, offset.page_id), offset_within_page)
}

/// Build an index of page start/end offsets for efficient position lookups (for token streams)
fn build_page_offsets_from_tokens(stream: &BookTokenStream) -> Vec<PageOffset> {
    let mut offsets = Vec::with_capacity(stream.pages.len());
    let mut current_offset = 0usize;

    for page in &stream.pages {
        let end_offset = current_offset + page.lemma_ids.len();
        offsets.push(PageOffset {
            part_index: page.part_index,
            page_id: page.page_id,
            start_offset: current_offset,
            end_offset,
        });
        current_offset = end_offset;
    }

    offsets
}

/// Find the page and offset within that page for a given global position (for token streams).
/// Uses binary search for efficiency with large books.
fn find_page_and_offset_tokens(
    page_offsets: &[PageOffset],
    _stream: &BookTokenStream,
    pos: usize,
) -> ((u32, u32), u32) {
    // Binary search to find the page containing this position
    let page_idx = page_offsets
        .binary_search_by(|offset| {
            if pos < offset.start_offset {
                std::cmp::Ordering::Greater
            } else if pos >= offset.end_offset {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .unwrap_or_else(|idx| idx.saturating_sub(1).min(page_offsets.len() - 1));

    let offset = &page_offsets[page_idx];
    let offset_within_page = (pos - offset.start_offset) as u32;

    ((offset.part_index, offset.page_id), offset_within_page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PageLemmas;

    fn create_test_stream(page_sizes: &[usize]) -> BookLemmaStream {
        let mut pages = Vec::new();
        let mut total_tokens = 0;
        let mut lemma_counter = 1u32;

        for (i, &size) in page_sizes.iter().enumerate() {
            let lemma_ids: Vec<u32> = (lemma_counter..lemma_counter + size as u32).collect();
            lemma_counter += size as u32;
            total_tokens += size;

            pages.push(PageLemmas {
                part_index: 1,
                page_id: i as u32 + 1,
                lemma_ids,
            });
        }

        BookLemmaStream {
            book_id: 1,
            total_tokens,
            pages,
        }
    }

    #[test]
    fn test_empty_stream() {
        let stream = BookLemmaStream {
            book_id: 1,
            total_tokens: 0,
            pages: vec![],
        };
        let params = ComparisonParams::default();
        let windows = generate_windows(&stream, &params);
        assert!(windows.is_empty());
    }

    #[test]
    fn test_small_stream_single_window() {
        let stream = create_test_stream(&[50]);
        let params = ComparisonParams {
            window_size: 275,
            stride: 60,
            min_length: 10,
            ..Default::default()
        };
        let windows = generate_windows(&stream, &params);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].lemma_ids.len(), 50);
        assert_eq!(windows[0].global_start, 0);
        assert_eq!(windows[0].global_end, 50);
    }

    #[test]
    fn test_exact_window_size() {
        let stream = create_test_stream(&[275]);
        let params = ComparisonParams {
            window_size: 275,
            stride: 275,
            min_length: 10,
            ..Default::default()
        };
        let windows = generate_windows(&stream, &params);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].lemma_ids.len(), 275);
    }

    #[test]
    fn test_multiple_windows() {
        // 500 tokens should give us multiple windows with stride 60
        let stream = create_test_stream(&[500]);
        let params = ComparisonParams {
            window_size: 275,
            stride: 60,
            min_length: 10,
            ..Default::default()
        };
        let windows = generate_windows(&stream, &params);

        // First window: 0-275
        // Second window: 60-335
        // Third window: 120-395
        // Fourth window: 180-455
        // Fifth window: 240-500 (partial, but >= min_length)
        assert!(windows.len() >= 4);

        // Verify first window
        assert_eq!(windows[0].global_start, 0);
        assert_eq!(windows[0].global_end, 275);

        // Verify second window
        assert_eq!(windows[1].global_start, 60);
        assert_eq!(windows[1].global_end, 335);
    }

    #[test]
    fn test_page_boundary_tracking() {
        // Create stream with multiple pages
        let stream = create_test_stream(&[100, 100, 100, 100]);
        let params = ComparisonParams {
            window_size: 150, // Spans multiple pages
            stride: 50,
            min_length: 10,
            ..Default::default()
        };
        let windows = generate_windows(&stream, &params);

        // First window should start in page 1 and end in page 2
        assert_eq!(windows[0].start_page, (1, 1));
        assert_eq!(windows[0].end_page, (1, 2));
    }

    #[test]
    fn test_window_idx_increments() {
        let stream = create_test_stream(&[500]);
        let params = ComparisonParams {
            window_size: 100,
            stride: 50,
            min_length: 10,
            ..Default::default()
        };
        let windows = generate_windows(&stream, &params);

        for (i, window) in windows.iter().enumerate() {
            assert_eq!(window.window_idx, i as u32);
        }
    }

    #[test]
    fn test_calculate_window_count() {
        let params = ComparisonParams {
            window_size: 275,
            stride: 60,
            min_length: 10,
            ..Default::default()
        };

        // Empty stream
        assert_eq!(calculate_window_count(0, &params), 0);

        // Small stream
        assert_eq!(calculate_window_count(50, &params), 1);

        // Exact window size
        assert_eq!(calculate_window_count(275, &params), 1);

        // Large stream
        let count = calculate_window_count(1000, &params);
        assert!(count > 1);
    }
}
