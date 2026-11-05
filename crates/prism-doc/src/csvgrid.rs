//! RFC 4180 CSV reading and writing that preserves field quoting.
//!
//! Prism distinguishes text cells from numeric cells by quoting them in
//! `data/tables/<uid>/data.csv`, so the generic CSV crates are not usable here:
//! they normalise quoting away. This parser reports it.

use crate::value::{Cell, Grid};

/// Parse a Prism `data.csv` into a grid, growing to `min_rows` x `min_cols`
/// so a table keeps the dimensions declared in its `content.json`.
pub fn parse(input: &str, min_rows: usize, min_cols: usize) -> Grid {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut row: Vec<Cell> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => {
                in_quotes = true;
                quoted = true;
            }
            ',' => {
                row.push(Cell::from_csv_field(&field, quoted));
                field.clear();
                quoted = false;
            }
            '\r' => {
                // Swallow; the following '\n' ends the record.
            }
            '\n' => {
                row.push(Cell::from_csv_field(&field, quoted));
                field.clear();
                quoted = false;
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    // A final record without a trailing newline.
    if !field.is_empty() || quoted || !row.is_empty() {
        row.push(Cell::from_csv_field(&field, quoted));
        rows.push(row);
    }

    let n_rows = rows.len().max(min_rows);
    let n_cols = rows.iter().map(Vec::len).max().unwrap_or(0).max(min_cols);
    let mut grid = Grid::new(n_rows, n_cols);
    for (r, cells) in rows.into_iter().enumerate() {
        for (c, cell) in cells.into_iter().enumerate() {
            grid.set(r, c, cell);
        }
    }
    grid
}

/// Render a grid back to the CSV layout Prism reads, quoting text cells.
pub fn write(grid: &Grid) -> String {
    let mut out = String::new();
    for r in 0..grid.rows() {
        for c in 0..grid.cols() {
            if c > 0 {
                out.push(',');
            }
            match grid.get(r, c) {
                Cell::Blank => {}
                Cell::Number(v) => out.push_str(&format_number(*v)),
                Cell::Text(s) => {
                    out.push('"');
                    for ch in s.chars() {
                        if ch == '"' {
                            out.push('"');
                        }
                        out.push(ch);
                    }
                    out.push('"');
                }
            }
        }
        out.push('\n');
    }
    out
}

/// Format a value the way Prism's own tables do: plain decimal where possible,
/// with enough digits to round-trip the stored `f64` exactly.
fn format_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let short = format!("{v}");
    if short.parse::<f64>() == Ok(v) {
        short
    } else {
        format!("{v:.17}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_separates_text_from_numbers() {
        let g = parse("\"Cairo, Egypt\",,0.89\n\"x\",1,2\n", 0, 0);
        assert_eq!(g.rows(), 2);
        assert_eq!(g.cols(), 3);
        assert_eq!(g.get(0, 0).as_text(), Some("Cairo, Egypt"));
        assert!(g.get(0, 1).is_blank());
        assert_eq!(g.get(0, 2).as_number(), Some(0.89));
    }

    #[test]
    fn escaped_quotes_survive_a_round_trip() {
        let g = parse("\"a\"\"b\",1\n", 0, 0);
        assert_eq!(g.get(0, 0).as_text(), Some("a\"b"));
        assert_eq!(write(&g), "\"a\"\"b\",1\n");
    }

    #[test]
    fn declared_dimensions_are_honoured() {
        let g = parse("1,2\n", 5, 4);
        assert_eq!((g.rows(), g.cols()), (5, 4));
    }

    #[test]
    fn values_round_trip_exactly() {
        let g = parse("22.09,0.231472\n23.80543,0.15383\n", 0, 0);
        assert_eq!(write(&g), "22.09,0.231472\n23.80543,0.15383\n");
    }
}
