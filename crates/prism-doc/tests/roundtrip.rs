//! Round-trip tests against the documents GraphPad Prism 11 ships inside its own
//! bundle. They are skipped when Prism is not installed, so the suite still runs
//! on a machine without it.

use std::path::{Path, PathBuf};

const BUNDLE: &str = "/Applications/Prism 11.app/Contents/SharedSupport";

fn samples(ext: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, ext, out);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new(BUNDLE), ext, &mut out);
    out.sort();
    out
}

#[test]
fn reads_every_shipped_prismt_template() {
    let files = samples("prismt");
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    for f in &files {
        let doc = prism_doc::open(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert!(
            !doc.data_sheets.is_empty() || !doc.graph_sheets.is_empty(),
            "{} parsed to an empty document",
            f.display()
        );
    }
    eprintln!("read {} .prismt templates", files.len());
}

#[test]
fn reads_every_shipped_pzfx_and_xml_sample() {
    let mut files = samples("pzfx");
    files.extend(samples("xml"));
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    let mut tables = 0;
    for f in &files {
        let doc = prism_doc::open(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        tables += doc.data_sheets.len();
    }
    assert!(tables > 0, "no data tables were read from {} files", files.len());
    eprintln!("read {} XML samples holding {tables} tables", files.len());
}

#[test]
fn prism_values_survive_a_write_and_reread() {
    let files = samples("prismt");
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    for f in &files {
        let before = prism_doc::open(f).unwrap();
        let bytes = prism_doc::prism::write_bytes(&before).unwrap();
        let after = prism_doc::prism::read_bytes(&bytes).unwrap();

        assert_eq!(
            before.data_sheets.len(),
            after.data_sheets.len(),
            "{}: sheet count changed",
            f.display()
        );
        for (a, b) in before.data_sheets.iter().zip(&after.data_sheets) {
            assert_eq!(a.title, b.title, "{}: title changed", f.display());
            assert_eq!(a.table.format, b.table.format, "{}: format changed", f.display());
            assert_eq!(
                a.table.values, b.table.values,
                "{} sheet {}: cell values changed",
                f.display(),
                a.title
            );
        }
    }
}

#[test]
fn pzfx_values_survive_a_write_and_reread() {
    let mut files = samples("pzfx");
    files.extend(samples("xml"));
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    for f in &files {
        let before = prism_doc::open(f).unwrap();
        let bytes = prism_doc::pzfx::write_bytes(&before).unwrap();
        let after = prism_doc::pzfx::read_bytes(&bytes).unwrap();

        assert_eq!(
            before.data_sheets.len(),
            after.data_sheets.len(),
            "{}: sheet count changed",
            f.display()
        );
        for (a, b) in before.data_sheets.iter().zip(&after.data_sheets) {
            assert_eq!(
                a.table.values, b.table.values,
                "{} sheet {}: cell values changed",
                f.display(),
                a.title
            );
            assert_eq!(
                a.table.ys.len(),
                b.table.ys.len(),
                "{} sheet {}: dataset count changed",
                f.display(),
                a.title
            );
        }
        // The graph template must survive, or saving would silently drop graphs.
        assert_eq!(
            before.preserved.entries.get(prism_doc::pzfx::TEMPLATE_KEY),
            after.preserved.entries.get(prism_doc::pzfx::TEMPLATE_KEY),
            "{}: template blob changed",
            f.display()
        );
    }
}

#[test]
fn row_titles_are_read_and_survive_a_round_trip() {
    let path = Path::new(BUNDLE).join("SampleData/Column/t test - Paired.xml");
    if !path.exists() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    let doc = prism_doc::open(&path).unwrap();
    let with_titles: Vec<&prism_doc::DataSheet> = doc
        .data_sheets
        .iter()
        .filter(|s| s.table.row_titles.iter().any(|t| !t.trim().is_empty()))
        .collect();
    assert!(
        !with_titles.is_empty(),
        "this sample's RowTitlesColumn holds subject names, so they must be read"
    );
    assert_eq!(
        with_titles[0].table.row_titles.first().map(String::as_str),
        Some("GS"),
        "row titles must keep their order"
    );

    let bytes = prism_doc::pzfx::write_bytes(&doc).unwrap();
    let back = prism_doc::pzfx::read_bytes(&bytes).unwrap();
    for (a, b) in doc.data_sheets.iter().zip(&back.data_sheets) {
        assert_eq!(a.table.row_titles, b.table.row_titles, "sheet {}", a.title);
        assert_eq!(a.table.values, b.table.values, "sheet {}", a.title);
    }
}

#[test]
fn converting_prism_to_pzfx_keeps_the_values() {
    let files = samples("prismt");
    if files.is_empty() {
        eprintln!("Prism 11 not installed; skipping");
        return;
    }
    let f = &files[0];
    let doc = prism_doc::open(f).unwrap();
    let xml = prism_doc::pzfx::write_bytes(&doc).unwrap();
    let back = prism_doc::pzfx::read_bytes(&xml).unwrap();
    assert_eq!(doc.data_sheets.len(), back.data_sheets.len());
    for (a, b) in doc.data_sheets.iter().zip(&back.data_sheets) {
        assert_eq!(a.table.values, b.table.values, "sheet {} changed", a.title);
    }
}
