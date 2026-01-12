//! SQLite database access layer for corpus.db

use crate::models::{
    BookInfo, BookLemmaStream, BookMetadata, BookTokenStream, CorpusStats, PageInfo, PageLemmas,
    PageTokens,
};
use calamine::{open_workbook, Reader, Xlsx};
use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Excel error: {0}")]
    Excel(#[from] calamine::Error),
    #[error("Excel XLSX error: {0}")]
    ExcelXlsx(#[from] calamine::XlsxError),
    #[error("Book not found: {0}")]
    BookNotFound(u32),
    #[error("Invalid token blob size")]
    InvalidTokenBlob,
}

/// Load token_id -> lemma_id mapping from token_definitions table.
/// This is ~1.8M entries, optimized for fast lookup using a flat array.
pub fn load_token_to_lemma(db_path: &Path) -> Result<Vec<u32>, DbError> {
    let conn = Connection::open(db_path)?;

    // Get max token ID to size the array
    let max_id: u32 =
        conn.query_row("SELECT MAX(id) FROM token_definitions", [], |row| {
            row.get(0)
        })?;

    // Pre-allocate array (index = token_id, value = lemma_id)
    let mut mapping = vec![0u32; (max_id + 1) as usize];

    let mut stmt = conn.prepare("SELECT id, lemma_id FROM token_definitions")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let token_id: u32 = row.get(0)?;
        let lemma_id: u32 = row.get(1)?;
        mapping[token_id as usize] = lemma_id;
    }

    Ok(mapping)
}

/// Load token_id -> root_id mapping from token_definitions table.
/// This is ~1.8M entries, optimized for fast lookup using a flat array.
/// root_id can be NULL in the database, in which case we use 0 (no root).
pub fn load_token_to_root(db_path: &Path) -> Result<Vec<u32>, DbError> {
    let conn = Connection::open(db_path)?;

    // Get max token ID to size the array
    let max_id: u32 =
        conn.query_row("SELECT MAX(id) FROM token_definitions", [], |row| {
            row.get(0)
        })?;

    // Pre-allocate array (index = token_id, value = root_id, 0 = no root)
    let mut mapping = vec![0u32; (max_id + 1) as usize];

    let mut stmt = conn.prepare("SELECT id, root_id FROM token_definitions")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let token_id: u32 = row.get(0)?;
        let root_id: Option<u32> = row.get(1)?;
        mapping[token_id as usize] = root_id.unwrap_or(0);
    }

    Ok(mapping)
}

/// Load token_id -> surface form mapping from token_definitions table.
/// This is ~1.8M entries, optimized for fast lookup using a flat array.
pub fn load_token_to_surface(db_path: &Path) -> Result<Vec<String>, DbError> {
    let conn = Connection::open(db_path)?;

    // Get max token ID to size the array
    let max_id: u32 =
        conn.query_row("SELECT MAX(id) FROM token_definitions", [], |row| {
            row.get(0)
        })?;

    // Pre-allocate array with empty strings (index = token_id, value = surface)
    let mut mapping = vec![String::new(); (max_id + 1) as usize];

    let mut stmt = conn.prepare("SELECT id, surface FROM token_definitions")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let token_id: u32 = row.get(0)?;
        let surface: String = row.get(1)?;
        mapping[token_id as usize] = surface;
    }

    Ok(mapping)
}

/// Load both token_to_lemma and token_to_surface mappings in a single pass.
/// More efficient than loading them separately when you need both.
pub fn load_token_mappings(db_path: &Path) -> Result<(Vec<u32>, Vec<String>), DbError> {
    let conn = Connection::open(db_path)?;

    // Get max token ID to size the arrays
    let max_id: u32 =
        conn.query_row("SELECT MAX(id) FROM token_definitions", [], |row| {
            row.get(0)
        })?;

    // Pre-allocate arrays
    let mut lemma_mapping = vec![0u32; (max_id + 1) as usize];
    let mut surface_mapping = vec![String::new(); (max_id + 1) as usize];

    let mut stmt = conn.prepare("SELECT id, surface, lemma_id FROM token_definitions")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let token_id: u32 = row.get(0)?;
        let surface: String = row.get(1)?;
        let lemma_id: u32 = row.get(2)?;
        lemma_mapping[token_id as usize] = lemma_id;
        surface_mapping[token_id as usize] = surface;
    }

    Ok((lemma_mapping, surface_mapping))
}

/// Load token_to_lemma, token_to_root, and token_to_surface mappings in a single pass.
/// Most efficient when you need all three mappings.
pub fn load_all_token_mappings(db_path: &Path) -> Result<(Vec<u32>, Vec<u32>, Vec<String>), DbError> {
    let conn = Connection::open(db_path)?;

    // Get max token ID to size the arrays
    let max_id: u32 =
        conn.query_row("SELECT MAX(id) FROM token_definitions", [], |row| {
            row.get(0)
        })?;

    // Pre-allocate arrays
    let mut lemma_mapping = vec![0u32; (max_id + 1) as usize];
    let mut root_mapping = vec![0u32; (max_id + 1) as usize];
    let mut surface_mapping = vec![String::new(); (max_id + 1) as usize];

    let mut stmt = conn.prepare("SELECT id, surface, lemma_id, root_id FROM token_definitions")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let token_id: u32 = row.get(0)?;
        let surface: String = row.get(1)?;
        let lemma_id: u32 = row.get(2)?;
        let root_id: Option<u32> = row.get(3)?;
        lemma_mapping[token_id as usize] = lemma_id;
        root_mapping[token_id as usize] = root_id.unwrap_or(0);
        surface_mapping[token_id as usize] = surface;
    }

    Ok((lemma_mapping, root_mapping, surface_mapping))
}

/// Load full token stream for a book (includes token_ids, lemma_ids, and root_ids).
/// Use this when you need text reconstruction capabilities or root-based matching.
pub fn load_book_token_stream(
    db_path: &Path,
    book_id: u32,
    token_to_lemma: &[u32],
) -> Result<BookTokenStream, DbError> {
    // Load root mapping as well
    let token_to_root = load_token_to_root(db_path)?;
    load_book_token_stream_with_root(db_path, book_id, token_to_lemma, &token_to_root)
}

/// Load full token stream for a book with pre-loaded root mapping.
/// Use this when you've already loaded token_to_root for efficiency.
pub fn load_book_token_stream_with_root(
    db_path: &Path,
    book_id: u32,
    token_to_lemma: &[u32],
    token_to_root: &[u32],
) -> Result<BookTokenStream, DbError> {
    let conn = Connection::open(db_path)?;

    let mut stmt = conn.prepare(
        "SELECT part_index, page_id, token_ids
         FROM page_tokens
         WHERE book_id = ?
         ORDER BY part_index, page_id",
    )?;

    let mut pages = Vec::new();
    let mut total_tokens = 0usize;

    let mut rows = stmt.query([book_id])?;

    while let Some(row) = rows.next()? {
        let part_index: u32 = row.get(0)?;
        let page_id: u32 = row.get(1)?;
        let token_blob: Vec<u8> = row.get(2)?;

        // Validate blob size is multiple of 4
        if token_blob.len() % 4 != 0 {
            return Err(DbError::InvalidTokenBlob);
        }

        // Unpack little-endian u32 array
        let token_ids: Vec<u32> = token_blob
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        // Map token_ids to lemma_ids
        let lemma_ids: Vec<u32> = token_ids
            .iter()
            .map(|&tid| {
                if (tid as usize) < token_to_lemma.len() {
                    token_to_lemma[tid as usize]
                } else {
                    0
                }
            })
            .collect();

        // Map token_ids to root_ids
        let root_ids: Vec<u32> = token_ids
            .iter()
            .map(|&tid| {
                if (tid as usize) < token_to_root.len() {
                    token_to_root[tid as usize]
                } else {
                    0
                }
            })
            .collect();

        total_tokens += token_ids.len();

        pages.push(PageTokens {
            part_index,
            page_id,
            token_ids,
            lemma_ids,
            root_ids,
        });
    }

    if pages.is_empty() {
        return Err(DbError::BookNotFound(book_id));
    }

    Ok(BookTokenStream {
        book_id,
        total_tokens,
        pages,
    })
}

/// Load lemma stream for a single book.
/// Extracts all token IDs from page_tokens and maps them to lemma IDs.
pub fn load_book_lemma_stream(
    db_path: &Path,
    book_id: u32,
    token_to_lemma: &[u32],
) -> Result<BookLemmaStream, DbError> {
    let conn = Connection::open(db_path)?;

    let mut stmt = conn.prepare(
        "SELECT part_index, page_id, token_ids
         FROM page_tokens
         WHERE book_id = ?
         ORDER BY part_index, page_id",
    )?;

    let mut pages = Vec::new();
    let mut total_tokens = 0usize;

    let mut rows = stmt.query([book_id])?;

    while let Some(row) = rows.next()? {
        let part_index: u32 = row.get(0)?;
        let page_id: u32 = row.get(1)?;
        let token_blob: Vec<u8> = row.get(2)?;

        // Validate blob size is multiple of 4
        if token_blob.len() % 4 != 0 {
            return Err(DbError::InvalidTokenBlob);
        }

        // Unpack little-endian u32 array
        let token_ids: Vec<u32> = token_blob
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        // Map token_ids to lemma_ids
        let lemma_ids: Vec<u32> = token_ids
            .iter()
            .filter_map(|&tid| {
                if (tid as usize) < token_to_lemma.len() {
                    Some(token_to_lemma[tid as usize])
                } else {
                    None // Skip invalid token IDs
                }
            })
            .collect();

        total_tokens += lemma_ids.len();

        pages.push(PageLemmas {
            part_index,
            page_id,
            lemma_ids,
        });
    }

    if pages.is_empty() {
        return Err(DbError::BookNotFound(book_id));
    }

    Ok(BookLemmaStream {
        book_id,
        total_tokens,
        pages,
    })
}

/// Load corpus statistics
pub fn load_corpus_stats(db_path: &Path) -> Result<CorpusStats, DbError> {
    let conn = Connection::open(db_path)?;

    let total_books: u64 = conn.query_row(
        "SELECT COUNT(DISTINCT book_id) FROM page_tokens",
        [],
        |row| row.get(0),
    )?;

    let total_pages: u64 =
        conn.query_row("SELECT COUNT(*) FROM page_tokens", [], |row| row.get(0))?;

    let total_tokens: u64 = conn.query_row(
        "SELECT SUM(LENGTH(token_ids) / 4) FROM page_tokens",
        [],
        |row| row.get(0),
    )?;

    let unique_lemmas: u64 =
        conn.query_row("SELECT COUNT(*) FROM lemmas", [], |row| row.get(0))?;

    let unique_roots: u64 =
        conn.query_row("SELECT COUNT(*) FROM roots", [], |row| row.get(0))?;

    let token_definitions: u64 = conn.query_row(
        "SELECT COUNT(*) FROM token_definitions",
        [],
        |row| row.get(0),
    )?;

    Ok(CorpusStats {
        total_books,
        total_pages,
        total_tokens,
        unique_lemmas,
        unique_roots,
        token_definitions,
    })
}

/// Load information about a specific book
pub fn load_book_info(db_path: &Path, book_id: u32) -> Result<BookInfo, DbError> {
    let conn = Connection::open(db_path)?;

    // Get page count and total tokens
    let (page_count, total_tokens): (u64, u64) = conn.query_row(
        "SELECT COUNT(*), SUM(LENGTH(token_ids) / 4)
         FROM page_tokens
         WHERE book_id = ?",
        [book_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    if page_count == 0 {
        return Err(DbError::BookNotFound(book_id));
    }

    // Get page details
    let mut stmt = conn.prepare(
        "SELECT pt.part_index, pt.page_id, LENGTH(pt.token_ids) / 4,
                p.part_label, p.page_number
         FROM page_tokens pt
         LEFT JOIN pages p ON pt.book_id = p.book_id
                          AND pt.part_index = p.part_index
                          AND pt.page_id = p.page_id
         WHERE pt.book_id = ?
         ORDER BY pt.part_index, pt.page_id",
    )?;

    let mut pages = Vec::new();
    let mut rows = stmt.query([book_id])?;

    while let Some(row) = rows.next()? {
        pages.push(PageInfo {
            book_id,
            part_index: row.get(0)?,
            page_id: row.get(1)?,
            token_count: row.get(2)?,
            part_label: row.get(3)?,
            page_number: row.get(4)?,
        });
    }

    // Count unique lemmas for this book
    let token_to_lemma = load_token_to_lemma(db_path)?;
    let stream = load_book_lemma_stream(db_path, book_id, &token_to_lemma)?;
    let unique_lemmas = {
        let mut lemmas: Vec<u32> = stream.flat_lemmas();
        lemmas.sort_unstable();
        lemmas.dedup();
        lemmas.len() as u64
    };

    Ok(BookInfo {
        book_id,
        page_count,
        total_tokens,
        unique_lemmas,
        pages,
    })
}

/// Load book metadata from Excel file
pub fn load_metadata_from_excel(
    excel_path: &Path,
) -> Result<HashMap<u32, BookMetadata>, DbError> {
    let mut workbook: Xlsx<_> = open_workbook(excel_path)?;
    let mut metadata = HashMap::new();

    // Try to find the first sheet
    if let Some(sheet_name) = workbook.sheet_names().first().cloned() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let mut rows = range.rows();

            // Skip header row
            let header = rows.next();

            // Find column indices from header
            let col_indices = if let Some(header_row) = header {
                find_column_indices(header_row)
            } else {
                return Ok(metadata);
            };

            for row in rows {
                if let Some(id) = get_u32_cell(row, col_indices.id) {
                    let book = BookMetadata {
                        id,
                        corpus: get_string_cell(row, col_indices.corpus).unwrap_or_default(),
                        title: get_string_cell(row, col_indices.title).unwrap_or_default(),
                        author_id: get_u32_cell(row, col_indices.author_id),
                        death_ah: get_u32_cell(row, col_indices.death_ah),
                        century_ah: get_u32_cell(row, col_indices.century_ah).map(|v| v as u8),
                        genre_id: get_u32_cell(row, col_indices.genre_id),
                        page_count: get_u32_cell(row, col_indices.page_count).unwrap_or(0),
                        token_count: get_u64_cell(row, col_indices.token_count).unwrap_or(0),
                    };
                    metadata.insert(id, book);
                }
            }
        }
    }

    Ok(metadata)
}

/// Column indices for metadata Excel file
struct ColumnIndices {
    id: Option<usize>,
    corpus: Option<usize>,
    title: Option<usize>,
    author_id: Option<usize>,
    death_ah: Option<usize>,
    century_ah: Option<usize>,
    genre_id: Option<usize>,
    page_count: Option<usize>,
    token_count: Option<usize>,
}

fn find_column_indices(header: &[calamine::Data]) -> ColumnIndices {
    let mut indices = ColumnIndices {
        id: None,
        corpus: None,
        title: None,
        author_id: None,
        death_ah: None,
        century_ah: None,
        genre_id: None,
        page_count: None,
        token_count: None,
    };

    for (i, cell) in header.iter().enumerate() {
        if let calamine::Data::String(s) = cell {
            let s_lower = s.to_lowercase();
            match s_lower.as_str() {
                "id" | "book_id" => indices.id = Some(i),
                "corpus" => indices.corpus = Some(i),
                "title" => indices.title = Some(i),
                "author_id" => indices.author_id = Some(i),
                "death_ah" => indices.death_ah = Some(i),
                "century_ah" => indices.century_ah = Some(i),
                "genre_id" => indices.genre_id = Some(i),
                "page_count" => indices.page_count = Some(i),
                "token_count" => indices.token_count = Some(i),
                _ => {}
            }
        }
    }

    indices
}

fn get_string_cell(row: &[calamine::Data], col: Option<usize>) -> Option<String> {
    col.and_then(|i| row.get(i)).and_then(|cell| match cell {
        calamine::Data::String(s) => Some(s.clone()),
        calamine::Data::Int(n) => Some(n.to_string()),
        calamine::Data::Float(n) => Some(n.to_string()),
        _ => None,
    })
}

fn get_u32_cell(row: &[calamine::Data], col: Option<usize>) -> Option<u32> {
    col.and_then(|i| row.get(i)).and_then(|cell| match cell {
        calamine::Data::Int(n) => Some(*n as u32),
        calamine::Data::Float(n) => Some(*n as u32),
        calamine::Data::String(s) => s.parse().ok(),
        _ => None,
    })
}

fn get_u64_cell(row: &[calamine::Data], col: Option<usize>) -> Option<u64> {
    col.and_then(|i| row.get(i)).and_then(|cell| match cell {
        calamine::Data::Int(n) => Some(*n as u64),
        calamine::Data::Float(n) => Some(*n as u64),
        calamine::Data::String(s) => s.parse().ok(),
        _ => None,
    })
}

/// Get lemma text for a given lemma ID
pub fn get_lemma_text(db_path: &Path, lemma_id: u32) -> Result<Option<String>, DbError> {
    let conn = Connection::open(db_path)?;
    let result: Option<String> = conn
        .query_row("SELECT lemma FROM lemmas WHERE id = ?", [lemma_id], |row| {
            row.get(0)
        })
        .ok();
    Ok(result)
}

/// Get multiple lemma texts for a slice of lemma IDs
pub fn get_lemma_texts(db_path: &Path, lemma_ids: &[u32]) -> Result<HashMap<u32, String>, DbError> {
    let conn = Connection::open(db_path)?;
    let mut lemmas = HashMap::new();

    // Use a prepared statement for efficiency
    let mut stmt = conn.prepare("SELECT lemma FROM lemmas WHERE id = ?")?;

    for &id in lemma_ids {
        if let Ok(lemma) = stmt.query_row([id], |row| row.get::<_, String>(0)) {
            lemmas.insert(id, lemma);
        }
    }

    Ok(lemmas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_blob_unpacking() {
        // Test that we correctly unpack little-endian u32 arrays
        let blob: Vec<u8> = vec![
            1, 0, 0, 0, // 1
            2, 0, 0, 0, // 2
            255, 0, 0, 0, // 255
        ];

        let tokens: Vec<u32> = blob
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        assert_eq!(tokens, vec![1, 2, 255]);
    }
}
