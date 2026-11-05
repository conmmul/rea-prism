//! Cell values and the dense grid that backs a Prism data table.
//!
//! Prism stores the cell values of a `.prism` table in `data/tables/<uid>/data.csv`,
//! one physical subcolumn per CSV field and one row per CSV record. Quoted fields
//! carry text, bare fields carry numbers, and empty fields are blanks. `content.json`
//! alongside it repeats the row and column counts, which we use to size the grid even
//! when trailing rows are entirely blank.

use std::fmt;

/// One cell of a data table.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Cell {
    /// No value entered. Prism treats these as missing, not as zero.
    #[default]
    Blank,
    Number(f64),
    /// A text entry. Used for row/label columns and for categorical variables.
    Text(String),
}

impl Cell {
    pub fn is_blank(&self) -> bool {
        matches!(self, Cell::Blank)
    }

    /// The numeric value, if this cell holds one. Text is *not* coerced: Prism
    /// keeps text and numeric entries distinct even when the text parses.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Cell::Number(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Cell::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Parse one CSV field. `quoted` reports whether the source field carried
    /// quotes, which is how the on-disk format separates text from numbers.
    pub fn from_csv_field(raw: &str, quoted: bool) -> Cell {
        if raw.is_empty() {
            return Cell::Blank;
        }
        if quoted {
            return Cell::Text(raw.to_string());
        }
        match raw.parse::<f64>() {
            Ok(v) => Cell::Number(v),
            // An unquoted field that will not parse is still meaningful content;
            // keep it rather than silently dropping the entry.
            Err(_) => Cell::Text(raw.to_string()),
        }
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cell::Blank => Ok(()),
            Cell::Number(v) => write!(f, "{v}"),
            Cell::Text(s) => write!(f, "{s}"),
        }
    }
}

/// A dense row-major grid of cells.
///
/// Columns here are *physical subcolumns*: an XY table with two Y datasets in
/// triplicate has one X subcolumn followed by six Y subcolumns, matching the CSV.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grid {
    rows: usize,
    cols: usize,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(rows: usize, cols: usize) -> Self {
        Grid {
            rows,
            cols,
            cells: vec![Cell::Blank; rows * cols],
        }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn get(&self, row: usize, col: usize) -> &Cell {
        if row < self.rows && col < self.cols {
            &self.cells[row * self.cols + col]
        } else {
            &Cell::Blank
        }
    }

    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        if row >= self.rows || col >= self.cols {
            self.resize(self.rows.max(row + 1), self.cols.max(col + 1));
        }
        self.cells[row * self.cols + col] = cell;
    }

    /// Grow or shrink the grid, preserving the cells that remain in range.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        let mut next = vec![Cell::Blank; rows * cols];
        for r in 0..rows.min(self.rows) {
            for c in 0..cols.min(self.cols) {
                next[r * cols + c] = self.cells[r * self.cols + c].clone();
            }
        }
        self.rows = rows;
        self.cols = cols;
        self.cells = next;
    }

    /// Every cell of one physical subcolumn, top to bottom.
    pub fn column(&self, col: usize) -> impl Iterator<Item = &Cell> {
        (0..self.rows).map(move |r| self.get(r, col))
    }

    /// The numeric values of one subcolumn, skipping blanks and text.
    pub fn numeric_column(&self, col: usize) -> impl Iterator<Item = f64> + '_ {
        self.column(col).filter_map(Cell::as_number)
    }

    /// The index of the last row holding any content, or `None` when empty.
    pub fn last_used_row(&self) -> Option<usize> {
        (0..self.rows).rev().find(|&r| (0..self.cols).any(|c| !self.get(r, c).is_blank()))
    }

    /// Drop trailing all-blank rows. Prism sizes tables generously; writing the
    /// padding back out would grow the file on every round trip.
    pub fn trim_trailing_blank_rows(&mut self) {
        let keep = self.last_used_row().map_or(0, |r| r + 1);
        if keep < self.rows {
            self.resize(keep, self.cols);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_stay_text_even_when_numeric() {
        assert_eq!(Cell::from_csv_field("12", true), Cell::Text("12".into()));
        assert_eq!(Cell::from_csv_field("12", false), Cell::Number(12.0));
        assert_eq!(Cell::from_csv_field("", false), Cell::Blank);
    }

    #[test]
    fn resize_preserves_overlap() {
        let mut g = Grid::new(2, 2);
        g.set(0, 0, Cell::Number(1.0));
        g.set(1, 1, Cell::Number(4.0));
        g.resize(3, 3);
        assert_eq!(g.get(0, 0), &Cell::Number(1.0));
        assert_eq!(g.get(1, 1), &Cell::Number(4.0));
        assert_eq!(g.get(2, 2), &Cell::Blank);
    }

    #[test]
    fn trailing_blank_rows_are_trimmed() {
        let mut g = Grid::new(10, 2);
        g.set(1, 0, Cell::Number(1.0));
        g.trim_trailing_blank_rows();
        assert_eq!(g.rows(), 2);
    }
}
