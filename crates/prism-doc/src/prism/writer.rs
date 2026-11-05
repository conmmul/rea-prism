use std::io::{Cursor, Write};
use std::path::Path;

use serde_json::{json, Map, Value};
use zip::write::SimpleFileOptions;

use crate::csvgrid;
use crate::error::{Error, Result};
use crate::model::*;

pub fn write_file(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let bytes = write_bytes(doc)?;
    std::fs::write(path, bytes).map_err(|source| Error::Io { path: path.into(), source })
}

pub fn write_bytes(doc: &Document) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        // Prism stores its container entries uncompressed; match that so the files
        // we write look to Prism exactly like the ones it writes itself.
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o666);

        let put = |zip: &mut zip::ZipWriter<Cursor<&mut Vec<u8>>>,
                       name: &str,
                       bytes: &[u8]|
         -> Result<()> {
            zip.start_file(name, opts)?;
            zip.write_all(bytes)?;
            Ok(())
        };

        put(&mut zip, "document.json", to_pretty(&manifest(doc)).as_bytes())?;

        for sheet in &doc.data_sheets {
            put(
                &mut zip,
                &format!("data/sheets/{}/sheet.json", sheet.uid),
                to_pretty(&data_sheet_json(sheet)).as_bytes(),
            )?;

            let t = &sheet.table;
            if let Some(x) = &t.x {
                put(
                    &mut zip,
                    &format!("data/sets/{}.json", x.uid),
                    to_pretty(&data_set_json(x, t, true)).as_bytes(),
                )?;
            }
            for y in &t.ys {
                put(
                    &mut zip,
                    &format!("data/sets/{}.json", y.uid),
                    to_pretty(&data_set_json(y, t, false)).as_bytes(),
                )?;
            }

            let csv = csvgrid::write(&t.values);
            put(
                &mut zip,
                &format!("data/tables/{}/content.json", t.uid),
                to_pretty(&json!({
                    "$id": "sheets/data_sheet/data_table/data_table_storage.schema.json",
                    "numberOfColumns": t.values.cols(),
                    "numberOfRows": t.values.rows(),
                    "version": 1
                }))
                .as_bytes(),
            )?;
            put(&mut zip, &format!("data/tables/{}/data.csv", t.uid), csv.as_bytes())?;
            put(
                &mut zip,
                &format!("data/tables/{}/data.dt", t.uid),
                content_token(&csv).as_bytes(),
            )?;
        }

        // Graphs, analyses, info sheets and side-car blobs pass through unchanged.
        for (name, bytes) in &doc.preserved.entries {
            put(&mut zip, name, bytes)?;
        }

        zip.finish()?;
    }
    Ok(buf)
}

/// Prism writes 32 hex characters to `data.dt` beside each table. It is not a
/// digest of `data.csv` under any common algorithm, and its construction is not
/// recovered, so we emit a stable token derived from the content: Prism rewrites
/// the value when it saves and does not reject the file over it.
fn content_token(csv: &str) -> String {
    // FNV-1a over the content, widened to 32 hex characters.
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    for b in csv.as_bytes() {
        h1 ^= *b as u64;
        h1 = h1.wrapping_mul(0x100_0000_01b3);
    }
    let mut h2: u64 = 0x9e37_79b9_7f4a_7c15 ^ csv.len() as u64;
    for b in csv.as_bytes().iter().rev() {
        h2 ^= *b as u64;
        h2 = h2.wrapping_mul(0x100_0000_01b3);
    }
    format!("{h1:016x}{h2:016x}")
}

fn to_pretty(v: &Value) -> String {
    // Prism indents with tabs; matching it keeps diffs between our output and
    // Prism's own readable.
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    use serde::Serialize;
    v.serialize(&mut ser).expect("serialising a Value cannot fail");
    String::from_utf8(out).expect("serde_json emits utf-8")
}

fn manifest(doc: &Document) -> Value {
    let mut attrs = Map::new();
    let mut add = |uid: &str, title: &str| {
        attrs.insert(uid.to_string(), json!({ "title": title }));
    };
    for s in &doc.data_sheets {
        add(&s.uid, &s.title);
    }
    for s in &doc.graph_sheets {
        add(&s.uid, &s.title);
    }
    for s in &doc.analyses {
        add(&s.uid, &s.title);
    }
    for s in &doc.info_sheets {
        add(&s.uid, &s.title);
    }

    let uids = |v: &[String]| Value::Array(v.iter().map(|u| json!(u)).collect());

    let mut root = Map::new();
    root.insert("@class".into(), json!("Document"));
    root.insert("$id".into(), json!("document.schema.json"));
    if let Some(d) = &doc.meta.created {
        root.insert("creationDate".into(), json!(d));
    }
    if let Some(d) = &doc.meta.modified {
        root.insert("modificationDate".into(), json!(d));
    }
    for (key, p) in [("createdBy", &doc.meta.created_by), ("modifiedBy", &doc.meta.modified_by)] {
        if let Some(p) = p {
            root.insert(
                key.into(),
                json!({
                    "name": p.name,
                    "user": p.user,
                    "version": p.version,
                    "platform": p.platform
                }),
            );
        }
    }
    root.insert("formatVersion".into(), json!(doc.meta.format_version));
    root.insert("minFormatVersion".into(), json!(doc.meta.min_format_version));
    root.insert("minPrismVersion".into(), json!(doc.meta.min_prism_version));
    root.insert(
        "sheets".into(),
        json!({
            "graphs": uids(&doc.graph_sheets.iter().map(|s| s.uid.clone()).collect::<Vec<_>>()),
            "analyses": uids(&doc.analyses.iter().map(|s| s.uid.clone()).collect::<Vec<_>>()),
            "data": uids(&doc.data_sheets.iter().map(|s| s.uid.clone()).collect::<Vec<_>>()),
        }),
    );
    if !doc.info_sheets.is_empty() {
        let info = uids(&doc.info_sheets.iter().map(|s| s.uid.clone()).collect::<Vec<_>>());
        root.get_mut("sheets").unwrap().as_object_mut().unwrap().insert("info".into(), info);
    }
    root.insert("sheetAttributesMap".into(), Value::Object(attrs));
    for (k, v) in &doc.preserved.document_extra {
        root.entry(k.clone()).or_insert_with(|| v.clone());
    }
    Value::Object(root)
}

fn data_sheet_json(sheet: &DataSheet) -> Value {
    let t = &sheet.table;
    let mut table = Map::new();
    table.insert(
        "@class".into(),
        json!(match t.format {
            TableFormat::XY => "XYDataTable",
            TableFormat::MultiVariable => "DataTable",
            _ => "DataTable",
        }),
    );
    table.insert("$id".into(), json!("sheets/data_sheet/data_table/data_table.schema.json"));
    table.insert("uid".into(), json!(t.uid));
    table.insert("format".into(), json!(t.format.as_json()));
    table.insert("dataFormat".into(), json!(t.data_format.as_json()));
    if t.data_format == SubcolumnFormat::Replicates {
        table.insert("replicatesCount".into(), json!(t.replicates));
    }
    table.insert(
        "dataSets".into(),
        Value::Array(t.ys.iter().map(|y| json!(y.uid)).collect()),
    );
    if let Some(x) = &t.x {
        table.insert("xDataSetFormat".into(), json!("y_single"));
        table.insert("xDataSet".into(), json!(x.uid));
    }

    let mut flags = Vec::new();
    if sheet.is_analysis_view {
        flags.push(json!("ANALYSIS_VIEW"));
    }

    let mut root = Map::new();
    root.insert("@class".into(), json!("DataSheet"));
    root.insert("$id".into(), json!("sheets/data_sheet/data_sheet.schema.json"));
    root.insert("uid".into(), json!(sheet.uid));
    root.insert("title".into(), json!(sheet.title));
    root.insert("table".into(), Value::Object(table));
    root.insert(
        "font".into(),
        json!({ "family": sheet.font.family, "size": sheet.font.size }),
    );
    if !flags.is_empty() {
        root.insert("flags".into(), Value::Array(flags));
    }
    root.insert("maxDataSets".into(), json!(2048));
    root.insert("maxRow".into(), json!(500000));
    Value::Object(root)
}

fn data_set_json(set: &DataSet, table: &DataTable, is_x: bool) -> Value {
    let width = if is_x { table.x_width().max(1) } else { table.y_width() };
    let last_row = table.values.last_used_row().map_or(0, |r| r as i64);
    let replicates: Vec<Value> = (0..width)
        .map(|_| {
            json!({
                "@class": "DriverReplicate",
                "valueType": "real",
                "commonAttributes": ["NUMERIC"],
                "decimalsLength": set.decimals.unwrap_or(3),
                "significantDigitsCount": 7,
                "firstRow": 0,
                "lastRow": last_row
            })
        })
        .collect();

    json!({
        "@class": "VariableDataSet",
        "$id": "sheets/data_sheet/data_table/data_set.schema.json",
        "uid": set.uid,
        "format": if is_x { "y_single" } else { table.data_format.as_json() },
        "title": set.title,
        "attributes": [if is_x { "DS_ATTR_X" } else { "DS_ATTR_Y" }],
        "replicates": replicates,
        "currentType": set.kind.as_json(),
        "isManual": true
    })
}
