use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use serde_json::Value;

use crate::csvgrid;
use crate::error::{Error, Result};
use crate::graph::GraphSpec;
use crate::model::*;
use crate::value::Grid;

pub fn read_file(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.into(), source })?;
    read_bytes(&bytes)
}

pub fn read_bytes(bytes: &[u8]) -> Result<Document> {
    let entries = unzip(bytes)?;
    let manifest = entries
        .get("document.json")
        .ok_or_else(|| Error::invalid("container has no document.json"))?;
    let manifest: Value = json(manifest, "document.json")?;

    let mut doc = Document {
        meta: read_meta(&manifest),
        ..Default::default()
    };

    let titles = sheet_titles(&manifest);

    // Data sheets, in the order the manifest lists them.
    for uid in manifest_uids(&manifest, "data") {
        let key = format!("data/sheets/{uid}/sheet.json");
        let Some(raw) = entries.get(&key) else { continue };
        let sheet: Value = json(raw, &key)?;
        doc.data_sheets.push(read_data_sheet(&uid, &sheet, &entries, &titles)?);
    }

    for uid in manifest_uids(&manifest, "graphs") {
        let key = format!("graphs/{uid}/sheet.json");
        let Some(raw) = entries.get(&key) else { continue };
        let sheet: Value = json(raw, &key)?;
        doc.graph_sheets.push(read_graph_sheet(&uid, &sheet, &entries, &titles, &doc.data_sheets));
    }

    for uid in manifest_uids(&manifest, "analyses") {
        let key = format!("analyses/{uid}/sheet.json");
        let Some(raw) = entries.get(&key) else { continue };
        let sheet: Value = json(raw, &key)?;
        doc.analyses.push(AnalysisSheet {
            uid: uid.clone(),
            title: str_at(&sheet, "title").unwrap_or_default(),
            analysis_class: str_at(&sheet, "analysisClass").unwrap_or_default(),
            parameters: entries
                .get(&format!("analyses/{uid}/parameters.json"))
                .and_then(|b| serde_json::from_slice(b).ok())
                .unwrap_or(Value::Null),
            results: entries
                .get(&format!("analyses/{uid}/results.json"))
                .and_then(|b| serde_json::from_slice(b).ok()),
        });
    }

    for uid in manifest_uids(&manifest, "info") {
        let key = format!("info/{uid}/sheet.json");
        let Some(raw) = entries.get(&key) else { continue };
        let sheet: Value = json(raw, &key)?;
        doc.info_sheets.push(InfoSheet {
            uid: uid.clone(),
            title: str_at(&sheet, "title").or_else(|| titles.get(&uid).cloned()).unwrap_or_default(),
            notes: str_at(&sheet, "notes").unwrap_or_default(),
            constants: Vec::new(),
        });
    }

    // Keep everything we did not model so a write loses nothing.
    doc.preserved.entries = entries
        .iter()
        .filter(|(k, _)| !crate::prism::is_modelled(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Value::Object(map) = &manifest {
        doc.preserved.document_extra = map
            .iter()
            .filter(|(k, _)| {
                !matches!(
                    k.as_str(),
                    "@class"
                        | "$id"
                        | "creationDate"
                        | "modificationDate"
                        | "createdBy"
                        | "modifiedBy"
                        | "formatVersion"
                        | "minFormatVersion"
                        | "minPrismVersion"
                        | "sheets"
                        | "sheetAttributesMap"
                )
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
    }

    Ok(doc)
}

// ---------------------------------------------------------------------------

fn unzip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut out = BTreeMap::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().to_string();
        let mut buf = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut buf)?;
        out.insert(name, buf);
    }
    Ok(out)
}

fn json(bytes: &[u8], entry: &str) -> Result<Value> {
    serde_json::from_slice(bytes).map_err(|source| Error::Json { entry: entry.into(), source })
}

fn str_at(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(str::to_string)
}

fn read_meta(manifest: &Value) -> Meta {
    let producer = |key: &str| -> Option<Producer> {
        let p = manifest.get(key)?;
        Some(Producer {
            name: str_at(p, "name").unwrap_or_default(),
            user: str_at(p, "user").unwrap_or_default(),
            version: str_at(p, "version").unwrap_or_default(),
            platform: str_at(p, "platform").unwrap_or_default(),
        })
    };
    let d = Meta::default();
    Meta {
        created: str_at(manifest, "creationDate"),
        modified: str_at(manifest, "modificationDate"),
        created_by: producer("createdBy"),
        modified_by: producer("modifiedBy"),
        format_version: str_at(manifest, "formatVersion").unwrap_or(d.format_version),
        min_format_version: str_at(manifest, "minFormatVersion").unwrap_or(d.min_format_version),
        min_prism_version: str_at(manifest, "minPrismVersion").unwrap_or(d.min_prism_version),
    }
}

/// Sheet uids of one kind, in manifest order.
fn manifest_uids(manifest: &Value, kind: &str) -> Vec<Uid> {
    manifest
        .get("sheets")
        .and_then(|s| s.get(kind))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Titles live in the manifest's `sheetAttributesMap`, keyed by sheet uid.
fn sheet_titles(manifest: &Value) -> BTreeMap<Uid, String> {
    manifest
        .get("sheetAttributesMap")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.get("title")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------

fn read_data_sheet(
    uid: &str,
    sheet: &Value,
    entries: &BTreeMap<String, Vec<u8>>,
    titles: &BTreeMap<Uid, String>,
) -> Result<DataSheet> {
    let t = sheet
        .get("table")
        .ok_or_else(|| Error::invalid(format!("data sheet {uid} has no table")))?;

    let format = str_at(t, "format")
        .and_then(|s| TableFormat::from_json(&s))
        .unwrap_or(TableFormat::XY);
    let data_format = str_at(t, "dataFormat")
        .and_then(|s| SubcolumnFormat::from_json(&s))
        .unwrap_or(SubcolumnFormat::Single);
    let replicates = t
        .get("replicatesCount")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let table_uid = str_at(t, "uid").unwrap_or_else(new_uid);

    let load_set = |set_uid: &str| -> Option<DataSet> {
        let raw = entries.get(&format!("data/sets/{set_uid}.json"))?;
        let v: Value = serde_json::from_slice(raw).ok()?;
        Some(DataSet {
            uid: set_uid.to_string(),
            title: str_at(&v, "title").unwrap_or_default(),
            kind: str_at(&v, "currentType")
                .map(|s| DataSetKind::from_json(&s))
                .unwrap_or_default(),
            decimals: v
                .get("replicates")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(|r| r.get("decimalsLength"))
                .and_then(Value::as_u64)
                .map(|n| n as u32),
            scientific: false,
        })
    };

    let x = str_at(t, "xDataSet").and_then(|u| load_set(&u));
    let ys: Vec<DataSet> = t
        .get("dataSets")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|u| load_set(u).unwrap_or_else(|| DataSet::new("")))
                .collect()
        })
        .unwrap_or_default();

    // Cell values live beside the table, keyed by the *table* uid, not the sheet uid.
    let (rows, cols) = entries
        .get(&format!("data/tables/{table_uid}/content.json"))
        .and_then(|b| serde_json::from_slice::<Value>(b).ok())
        .map(|c| {
            (
                c.get("numberOfRows").and_then(Value::as_u64).unwrap_or(0) as usize,
                c.get("numberOfColumns").and_then(Value::as_u64).unwrap_or(0) as usize,
            )
        })
        .unwrap_or((0, 0));
    let values = entries
        .get(&format!("data/tables/{table_uid}/data.csv"))
        .map(|b| csvgrid::parse(&String::from_utf8_lossy(b), rows, cols))
        .unwrap_or_else(|| Grid::new(rows, cols));

    let flags: Vec<&str> = sheet
        .get("flags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let x_format = if format.has_x() {
        if x.is_some() { XFormat::Numbers } else { XFormat::None }
    } else {
        XFormat::None
    };

    Ok(DataSheet {
        uid: uid.to_string(),
        title: str_at(sheet, "title")
            .or_else(|| titles.get(uid).cloned())
            .unwrap_or_default(),
        table: DataTable {
            uid: table_uid,
            format,
            data_format,
            x_format,
            replicates,
            series_start: None,
            series_interval: None,
            x_subcolumns: if x.is_some() { 1 } else { 0 },
            x_advanced: false,
            x,
            ys,
            row_titles: Vec::new(),
            values,
        },
        font: sheet
            .get("font")
            .map(|f| Font {
                family: str_at(f, "family").unwrap_or_else(|| "Arial".into()),
                size: f.get("size").and_then(Value::as_f64).unwrap_or(10.0) as f32,
            })
            .unwrap_or_default(),
        is_analysis_view: flags.contains(&"ANALYSIS_VIEW"),
    })
}

fn read_graph_sheet(
    uid: &str,
    sheet: &Value,
    entries: &BTreeMap<String, Vec<u8>>,
    titles: &BTreeMap<Uid, String>,
    data_sheets: &[DataSheet],
) -> GraphSheet {
    let class = str_at(sheet, "@class").unwrap_or_default();
    let kind = if class == "FENGraphSheet" { GraphKind::Classic } else { GraphKind::MultiVariable };

    let input_data_sets: Vec<Uid> = sheet
        .get("inputDataSets")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();

    let data_sheet = sheet
        .get("settings")
        .and_then(|s| s.get("graph"))
        .and_then(|g| g.get("dataSheet"))
        .and_then(Value::as_str)
        .map(str::to_string);

    // Find the table these datasets come from so we can build a faithful default.
    let source = data_sheets.iter().find(|ds| {
        data_sheet.as_deref() == Some(ds.uid.as_str())
            || input_data_sets.iter().any(|u| ds.table.ys.iter().any(|y| &y.uid == u))
    });

    let spec = match source {
        Some(ds) => {
            let mut s = GraphSpec::default_for_table(&ds.table);
            s.source = Some(ds.uid.clone());
            s
        }
        None => GraphSpec::default(),
    };

    GraphSheet {
        uid: uid.to_string(),
        title: str_at(sheet, "title")
            .or_else(|| titles.get(uid).cloned())
            .unwrap_or_default(),
        kind,
        input_data_sets,
        data_sheet,
        spec,
        legacy_blob: entries.get(&format!("graphs/{uid}/data.bin")).cloned(),
        raw_json: Some(sheet.clone()),
    }
}
