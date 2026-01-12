//! Lemma stream extraction utilities.
//!
//! Provides functions to extract and manipulate lemma streams from books.

use crate::db::{load_book_lemma_stream, load_token_to_lemma, DbError};
use crate::models::BookLemmaStream;
use std::path::Path;

/// Extract lemma stream for a book with a fresh database connection.
pub fn extract_book_lemmas(db_path: &Path, book_id: u32) -> Result<BookLemmaStream, DbError> {
    let token_to_lemma = load_token_to_lemma(db_path)?;
    load_book_lemma_stream(db_path, book_id, &token_to_lemma)
}

/// Extract lemma streams for multiple books, reusing the token-to-lemma mapping.
pub fn extract_books_lemmas(
    db_path: &Path,
    book_ids: &[u32],
) -> Result<Vec<BookLemmaStream>, DbError> {
    let token_to_lemma = load_token_to_lemma(db_path)?;

    let mut streams = Vec::with_capacity(book_ids.len());
    for &book_id in book_ids {
        let stream = load_book_lemma_stream(db_path, book_id, &token_to_lemma)?;
        streams.push(stream);
    }

    Ok(streams)
}

/// Get a slice of lemmas from a book at a specific global position range.
pub fn get_lemma_slice(stream: &BookLemmaStream, start: usize, end: usize) -> Vec<u32> {
    let flat = stream.flat_lemmas();
    if start >= flat.len() {
        return Vec::new();
    }
    let end = end.min(flat.len());
    flat[start..end].to_vec()
}

/// Find the position in the lemma stream for a given page location.
pub fn find_position_by_page(
    stream: &BookLemmaStream,
    part_index: u32,
    page_id: u32,
) -> Option<usize> {
    let mut position = 0;
    for page in &stream.pages {
        if page.part_index == part_index && page.page_id == page_id {
            return Some(position);
        }
        position += page.lemma_ids.len();
    }
    None
}

/// Get lemma IDs for a specific page.
pub fn get_page_lemmas(
    stream: &BookLemmaStream,
    part_index: u32,
    page_id: u32,
) -> Option<&[u32]> {
    stream
        .pages
        .iter()
        .find(|p| p.part_index == part_index && p.page_id == page_id)
        .map(|p| p.lemma_ids.as_slice())
}

/// Calculate statistics for a lemma stream.
pub struct LemmaStats {
    pub total_tokens: usize,
    pub unique_lemmas: usize,
    pub page_count: usize,
    pub avg_tokens_per_page: f64,
    pub most_common_lemma: Option<(u32, usize)>,
}

/// Calculate statistics for a lemma stream.
pub fn calculate_lemma_stats(stream: &BookLemmaStream) -> LemmaStats {
    let flat = stream.flat_lemmas();

    // Count unique lemmas and find most common
    let mut counts = std::collections::HashMap::new();
    for &lemma in &flat {
        *counts.entry(lemma).or_insert(0usize) += 1;
    }

    let unique_lemmas = counts.len();
    let most_common_lemma = counts
        .into_iter()
        .max_by_key(|&(_, count)| count);

    LemmaStats {
        total_tokens: stream.total_tokens,
        unique_lemmas,
        page_count: stream.pages.len(),
        avg_tokens_per_page: if stream.pages.is_empty() {
            0.0
        } else {
            stream.total_tokens as f64 / stream.pages.len() as f64
        },
        most_common_lemma,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PageLemmas;

    fn create_test_stream() -> BookLemmaStream {
        BookLemmaStream {
            book_id: 1,
            total_tokens: 30,
            pages: vec![
                PageLemmas {
                    part_index: 1,
                    page_id: 1,
                    lemma_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                },
                PageLemmas {
                    part_index: 1,
                    page_id: 2,
                    lemma_ids: vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
                },
                PageLemmas {
                    part_index: 2,
                    page_id: 1,
                    lemma_ids: vec![21, 22, 23, 24, 25, 26, 27, 28, 29, 30],
                },
            ],
        }
    }

    #[test]
    fn test_get_lemma_slice() {
        let stream = create_test_stream();

        let slice = get_lemma_slice(&stream, 5, 15);
        assert_eq!(slice, vec![6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);

        let slice = get_lemma_slice(&stream, 0, 5);
        assert_eq!(slice, vec![1, 2, 3, 4, 5]);

        let slice = get_lemma_slice(&stream, 25, 35); // Past end
        assert_eq!(slice, vec![26, 27, 28, 29, 30]);

        let slice = get_lemma_slice(&stream, 100, 110); // Way past end
        assert!(slice.is_empty());
    }

    #[test]
    fn test_find_position_by_page() {
        let stream = create_test_stream();

        assert_eq!(find_position_by_page(&stream, 1, 1), Some(0));
        assert_eq!(find_position_by_page(&stream, 1, 2), Some(10));
        assert_eq!(find_position_by_page(&stream, 2, 1), Some(20));
        assert_eq!(find_position_by_page(&stream, 3, 1), None); // Not found
    }

    #[test]
    fn test_get_page_lemmas() {
        let stream = create_test_stream();

        let lemmas = get_page_lemmas(&stream, 1, 1);
        assert!(lemmas.is_some());
        assert_eq!(lemmas.unwrap(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let lemmas = get_page_lemmas(&stream, 2, 1);
        assert!(lemmas.is_some());
        assert_eq!(lemmas.unwrap().len(), 10);

        let lemmas = get_page_lemmas(&stream, 3, 1);
        assert!(lemmas.is_none());
    }

    #[test]
    fn test_calculate_lemma_stats() {
        let stream = create_test_stream();
        let stats = calculate_lemma_stats(&stream);

        assert_eq!(stats.total_tokens, 30);
        assert_eq!(stats.unique_lemmas, 30);
        assert_eq!(stats.page_count, 3);
        assert!((stats.avg_tokens_per_page - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_lemma_stats_with_duplicates() {
        let stream = BookLemmaStream {
            book_id: 1,
            total_tokens: 10,
            pages: vec![PageLemmas {
                part_index: 1,
                page_id: 1,
                lemma_ids: vec![1, 1, 1, 2, 2, 3, 4, 5, 5, 5],
            }],
        };
        let stats = calculate_lemma_stats(&stream);

        assert_eq!(stats.total_tokens, 10);
        assert_eq!(stats.unique_lemmas, 5);

        // Most common should be 1 or 5 (both appear 3 times)
        let (most_common, count) = stats.most_common_lemma.unwrap();
        assert_eq!(count, 3);
        assert!(most_common == 1 || most_common == 5);
    }
}
