use std::fmt::Write as _;
use std::path::Path;

use crate::error::{Error, Result};
use crate::model::*;
use crate::value::Cell;

pub fn write_file(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let xml = write_bytes(doc)?;
    std::fs::write(path, xml).map_err(|source| Error::Io { path: path.into(), source })
}

pub fn write_bytes(doc: &Document) -> Result<Vec<u8>> {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        s,
        "<GraphPadPrismFile xmlns=\"{}\" PrismXMLVersion=\"5.00\">",
        super::NS
    )
    .unwrap();

    if let Some(p) = &doc.meta.created_by {
        s.push_str("<Created>\n");
        writeln!(
            s,
            "<OriginalVersion CreatedByProgram=\"{}\" CreatedByVersion=\"{}\"/>",
            escape(&p.name),
            escape(&p.version)
        )
        .unwrap();
        s.push_str("</Created>\n");
    }

    // The sequence element fixes the order sheets appear in Prism's navigator.
    s.push_str("<TableSequence>\n");
    for sheet in &doc.data_sheets {
        writeln!(s, "<Ref ID=\"{}\"/>", escape(&sheet.uid)).unwrap();
    }
    s.push_str("</TableSequence>\n");

    for sheet in &doc.data_sheets {
        write_table(&mut s, sheet);
    }

    if let Some(d) = doc.preserved.entries.get(super::TEMPLATE_DESCRIPTION_KEY) {
        writeln!(
            s,
            "<TemplateDescription>{}</TemplateDescription>",
            escape(&String::from_utf8_lossy(d))
        )
        .unwrap();
    }
    if let Some(t) = doc.preserved.entries.get(super::TEMPLATE_KEY) {
        writeln!(
            s,
            "<Template dt:dt=\"bin.base64\" xmlns:dt=\"urn:schemas-microsoft-com:datatypes\">{}</Template>",
            String::from_utf8_lossy(t)
        )
        .unwrap();
    }

    s.push_str("</GraphPadPrismFile>\n");
    Ok(s.into_bytes())
}

fn write_table(s: &mut String, sheet: &DataSheet) {
    let t = &sheet.table;
    let table_type = match t.format {
        TableFormat::XY | TableFormat::MultiVariable | TableFormat::Nested => "XY",
        TableFormat::Column => "OneWay",
        TableFormat::Grouped => "TwoWay",
        TableFormat::Contingency => "Contingency",
        TableFormat::Survival => "Survival",
        TableFormat::PartsOfWhole => "PartsOfWhole",
    };

    write!(
        s,
        "<Table ID=\"{}\" XFormat=\"{}\" YFormat=\"{}\"",
        escape(&sheet.uid),
        t.x_format.as_xml(),
        t.data_format.as_xml()
    )
    .unwrap();
    if t.data_format == SubcolumnFormat::Replicates {
        write!(s, " Replicates=\"{}\"", t.replicates).unwrap();
    }
    write!(s, " TableType=\"{table_type}\"").unwrap();
    match t.format {
        TableFormat::MultiVariable => s.push_str(" ExtTableType=\"MultipleVariables\""),
        TableFormat::Nested => s.push_str(" ExtTableType=\"Nested\""),
        _ => {}
    }
    s.push_str(">\n");

    writeln!(s, "<Title>{}</Title>", escape(&sheet.title)).unwrap();

    if !t.row_titles.is_empty() {
        s.push_str("<RowTitlesColumn Width=\"39\">\n<Subcolumn>\n");
        for title in &t.row_titles {
            writeln!(s, "<d>{}</d>", escape(title)).unwrap();
        }
        s.push_str("</Subcolumn>\n</RowTitlesColumn>\n");
    }

    let x_width = t.x_width();
    if let Some(x) = &t.x {
        // Date and time tables split X across `XColumn` (the elapsed number) and
        // `XAdvancedColumn` (the text Prism displays); plain tables use one element.
        let plain = if t.x_advanced { 1 } else { x_width.max(1) };
        writeln!(
            s,
            "<XColumn Width=\"89\" Decimals=\"{}\" Subcolumns=\"{}\">",
            x.decimals.unwrap_or(0),
            plain
        )
        .unwrap();
        if !x.title.is_empty() {
            writeln!(s, "<Title>{}</Title>", escape(&x.title)).unwrap();
        }
        for i in 0..plain {
            write_subcolumn(s, t, i);
        }
        s.push_str("</XColumn>\n");

        if t.x_advanced {
            let advanced = x_width.saturating_sub(1).max(1);
            writeln!(
                s,
                "<XAdvancedColumn Version=\"1\" Width=\"89\" Decimals=\"15\" Subcolumns=\"{advanced}\">"
            )
            .unwrap();
            for i in 0..advanced {
                write_subcolumn(s, t, plain + i);
            }
            s.push_str("</XAdvancedColumn>\n");
        }
    }

    let y_width = t.y_width();
    for (i, y) in t.ys.iter().enumerate() {
        writeln!(
            s,
            "<YColumn Width=\"89\" Decimals=\"{}\" Subcolumns=\"{}\">",
            y.decimals.unwrap_or(3),
            y_width
        )
        .unwrap();
        if !y.title.is_empty() {
            writeln!(s, "<Title>{}</Title>", escape(&y.title)).unwrap();
        }
        for k in 0..y_width {
            write_subcolumn(s, t, t.y_offset(i) + k);
        }
        s.push_str("</YColumn>\n");
    }

    s.push_str("</Table>\n");
}

fn write_subcolumn(s: &mut String, t: &DataTable, col: usize) {
    s.push_str("<Subcolumn>\n");
    let rows = t.values.last_used_row().map_or(0, |r| r + 1);
    for r in 0..rows {
        match t.values.get(r, col) {
            Cell::Blank => s.push_str("<d/>\n"),
            cell => {
                writeln!(s, "<d>{}</d>", escape(&cell.to_string())).unwrap();
            }
        }
    }
    s.push_str("</Subcolumn>\n");
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}
