use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{Error, Result};
use crate::model::*;
use crate::value::{Cell, Grid};

pub fn read_file(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.into(), source })?;
    read_bytes(&bytes)
}

/// Columns are accumulated here before we know the table's final shape.
#[derive(Default)]
struct PendingColumn {
    title: String,
    /// One vector of cells per subcolumn.
    subcolumns: Vec<Vec<Cell>>,
    decimals: Option<u32>,
    is_x: bool,
}

#[derive(Default)]
struct PendingTable {
    id: String,
    title: String,
    x_format: XFormat,
    y_format: String,
    replicates: usize,
    table_type: String,
    ext_table_type: String,
    start_at: Option<f64>,
    interval: Option<f64>,
    columns: Vec<PendingColumn>,
    row_titles: Vec<String>,
    /// Set when the table carried an `XAdvancedColumn` beside its `XColumn`.
    x_advanced: bool,
}

pub fn read_bytes(bytes: &[u8]) -> Result<Document> {
    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = false;

    let mut doc = Document::default();
    let mut tables: Vec<PendingTable> = Vec::new();

    // Parser state.
    let mut table: Option<PendingTable> = None;
    let mut column: Option<PendingColumn> = None;
    let mut subcolumn: Option<Vec<Cell>> = None;
    // Depth of nesting inside a text-bearing element, so that the inline markup
    // the schema allows (`<B>`, `<Font>`, `<I>`, ...) contributes its text and
    // nothing else.
    let mut text_sink: Option<TextSink> = None;
    let mut template = String::new();
    let mut template_description = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "GraphPadPrismFile" => {
                        for a in e.attributes() {
                            let a = a?;
                            if local_name(a.key.as_ref()) == "CreatedByPrismVersion" {
                                let v = a.unescape_value()?.to_string();
                                doc.meta.created_by = Some(Producer {
                                    name: "GraphPad Prism".into(),
                                    version: v,
                                    ..Default::default()
                                });
                            }
                        }
                    }
                    "Table" | "HugeTable" => {
                        let mut t = PendingTable { replicates: 1, ..Default::default() };
                        for a in e.attributes() {
                            let a = a?;
                            let key = local_name(a.key.as_ref());
                            let val = a.unescape_value()?.to_string();
                            match key.as_str() {
                                "ID" => t.id = val,
                                "XFormat" => t.x_format = XFormat::from_xml(&val),
                                "YFormat" => t.y_format = val,
                                "Replicates" => t.replicates = val.parse().unwrap_or(1).max(1),
                                "TableType" => t.table_type = val,
                                "ExtTableType" => t.ext_table_type = val,
                                "StartAt" => t.start_at = val.parse().ok(),
                                "Interval" => t.interval = val.parse().ok(),
                                _ => {}
                            }
                        }
                        table = Some(t);
                    }
                    // The row-titles column needs its own pending column: without one
                    // its subcolumn has nowhere to land and the titles are dropped.
                    "RowTitlesColumn" => column = Some(PendingColumn::default()),
                    "XColumn" | "XAdvancedColumn" | "YColumn" => {
                        if name == "XAdvancedColumn" {
                            if let Some(t) = table.as_mut() {
                                t.x_advanced = true;
                            }
                        }
                        let mut c = PendingColumn {
                            is_x: name != "YColumn",
                            ..Default::default()
                        };
                        for a in e.attributes() {
                            let a = a?;
                            if local_name(a.key.as_ref()) == "Decimals" {
                                c.decimals = a.unescape_value()?.parse().ok();
                            }
                        }
                        column = Some(c);
                    }
                    "Subcolumn" => subcolumn = Some(Vec::new()),
                    "Title" if table.is_some() => {
                        text_sink = Some(if column.is_some() {
                            TextSink::ColumnTitle
                        } else {
                            TextSink::TableTitle
                        });
                    }
                    "d" => text_sink = Some(TextSink::Cell { excluded: is_excluded(&e)? }),
                    "Template" => text_sink = Some(TextSink::Template),
                    "TemplateDescription" => text_sink = Some(TextSink::TemplateDescription),
                    _ => {}
                }
            }
            Event::Empty(e) => {
                // A self-closing `<d/>` is an empty cell, and `<BR/>` a line break.
                let name = local_name(e.name().as_ref());
                if name == "d" {
                    if let Some(sc) = subcolumn.as_mut() {
                        sc.push(Cell::Blank);
                    }
                } else if name == "BR" || name == "br" {
                    if let Some(TextSink::TableTitle | TextSink::ColumnTitle) = text_sink {
                        // Keep multi-line titles readable on one line.
                        push_text(&mut table, &mut column, &text_sink, " ");
                    }
                }
            }
            Event::Text(e) => {
                let raw = e.unescape()?.to_string();
                match text_sink {
                    Some(TextSink::Template) => template.push_str(raw.trim()),
                    Some(TextSink::TemplateDescription) => template_description.push_str(&raw),
                    Some(TextSink::Cell { excluded }) => {
                        if let Some(sc) = subcolumn.as_mut() {
                            let t = raw.trim();
                            // Excluded values stay in the table but are not analysed;
                            // we keep the value and note the exclusion is unmodelled.
                            let _ = excluded;
                            sc.push(if t.is_empty() {
                                Cell::Blank
                            } else {
                                Cell::from_csv_field(t, false)
                            });
                        }
                    }
                    Some(_) => push_text(&mut table, &mut column, &text_sink, &raw),
                    None => {}
                }
            }
            Event::End(e) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "Table" | "HugeTable" => {
                        if let Some(t) = table.take() {
                            tables.push(t);
                        }
                    }
                    "RowTitlesColumn" => {
                        // Its single subcolumn holds the row titles.
                        if let Some(c) = column.take() {
                            if let (Some(t), Some(first)) = (table.as_mut(), c.subcolumns.first()) {
                                t.row_titles = first.iter().map(|c| c.to_string()).collect();
                            }
                        }
                    }
                    "XColumn" | "XAdvancedColumn" | "YColumn" => {
                        if let (Some(t), Some(c)) = (table.as_mut(), column.take()) {
                            t.columns.push(c);
                        }
                    }
                    "Subcolumn" => {
                        if let Some(sc) = subcolumn.take() {
                            if let Some(c) = column.as_mut() {
                                c.subcolumns.push(sc);
                            }
                        }
                    }
                    "Title" | "d" | "Template" | "TemplateDescription" => text_sink = None,
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    for t in tables {
        doc.data_sheets.push(build_sheet(t));
    }
    if !template.is_empty() {
        doc.preserved
            .entries
            .insert(super::TEMPLATE_KEY.into(), template.into_bytes());
    }
    if !template_description.trim().is_empty() {
        doc.preserved.entries.insert(
            super::TEMPLATE_DESCRIPTION_KEY.into(),
            template_description.trim().as_bytes().to_vec(),
        );
    }
    Ok(doc)
}

#[derive(Clone, Copy)]
enum TextSink {
    TableTitle,
    ColumnTitle,
    Cell { excluded: bool },
    Template,
    TemplateDescription,
}

fn push_text(
    table: &mut Option<PendingTable>,
    column: &mut Option<PendingColumn>,
    sink: &Option<TextSink>,
    text: &str,
) {
    match sink {
        Some(TextSink::TableTitle) => {
            if let Some(t) = table.as_mut() {
                t.title.push_str(text);
            }
        }
        Some(TextSink::ColumnTitle) => {
            if let Some(c) = column.as_mut() {
                c.title.push_str(text);
            }
        }
        _ => {}
    }
}

fn is_excluded(e: &quick_xml::events::BytesStart) -> Result<bool> {
    for a in e.attributes() {
        let a = a?;
        if local_name(a.key.as_ref()) == "Excluded" {
            let v = a.unescape_value()?;
            return Ok(v == "1" || v.eq_ignore_ascii_case("true"));
        }
    }
    Ok(false)
}

fn local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

fn build_sheet(t: PendingTable) -> DataSheet {
    let format = match (t.ext_table_type.as_str(), t.table_type.as_str()) {
        ("MultipleVariables", _) => TableFormat::MultiVariable,
        ("Nested", _) => TableFormat::Nested,
        (_, "OneWay") => TableFormat::Column,
        (_, "TwoWay") => TableFormat::Grouped,
        (_, "Contingency") => TableFormat::Contingency,
        (_, "Survival") => TableFormat::Survival,
        (_, "PartsOfWhole") => TableFormat::PartsOfWhole,
        _ => TableFormat::XY,
    };
    let data_format = SubcolumnFormat::from_xml(&t.y_format, t.replicates);

    let (x_cols, y_cols): (Vec<_>, Vec<_>) = t.columns.into_iter().partition(|c| c.is_x);

    let x_set = x_cols.first().map(|c| DataSet {
        uid: new_uid(),
        title: c.title.trim().to_string(),
        kind: if t.x_format == XFormat::Text { DataSetKind::Label } else { DataSetKind::Continuous },
        decimals: c.decimals,
        scientific: false,
    });
    let ys: Vec<DataSet> = y_cols
        .iter()
        .map(|c| DataSet {
            uid: new_uid(),
            title: c.title.trim().to_string(),
            kind: DataSetKind::Continuous,
            decimals: c.decimals,
            scientific: false,
        })
        .collect();

    // Lay the subcolumns out the way the `.prism` grid expects: X first, then each
    // Y dataset's subcolumns in order.
    let mut columns: Vec<&Vec<Cell>> = Vec::new();
    for c in &x_cols {
        columns.extend(c.subcolumns.iter());
    }
    for c in &y_cols {
        columns.extend(c.subcolumns.iter());
    }
    let rows = columns.iter().map(|c| c.len()).max().unwrap_or(0);
    let mut values = Grid::new(rows, columns.len());
    for (ci, col) in columns.iter().enumerate() {
        for (ri, cell) in col.iter().enumerate() {
            values.set(ri, ci, cell.clone());
        }
    }

    let x_width = x_cols.iter().map(|c| c.subcolumns.len()).sum::<usize>();
    // Two plain numeric X subcolumns mean the table carries X error values.
    let x_format = if x_width >= 2 && t.x_format == XFormat::Numbers && !t.x_advanced {
        XFormat::Error
    } else if x_cols.is_empty() {
        XFormat::None
    } else {
        t.x_format
    };

    DataSheet {
        uid: if t.id.is_empty() { new_uid() } else { t.id.clone() },
        title: t.title.trim().to_string(),
        table: DataTable {
            uid: new_uid(),
            format,
            data_format,
            x_format,
            replicates: t.replicates,
            series_start: t.start_at,
            series_interval: t.interval,
            x_subcolumns: x_width,
            x_advanced: t.x_advanced,
            x: x_set,
            ys,
            row_titles: t.row_titles,
            values,
        },
        font: Font::default(),
        is_analysis_view: t.table_type == "Result",
    }
}
