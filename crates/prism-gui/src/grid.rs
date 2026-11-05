//! The spreadsheet view of a data sheet.

use egui::{Color32, Key, RichText, Sense, TextEdit, Vec2};
use prism_doc::model::{DataSheet, SubcolumnFormat};
use prism_doc::value::Cell;

const ROW_H: f32 = 22.0;
const COL_W: f32 = 92.0;
const HEAD_W: f32 = 56.0;

/// Which cell the user is typing into, and what they have typed so far.
#[derive(Default)]
pub struct Editing {
    pub cell: Option<(usize, usize)>,
    pub buffer: String,
}

/// Draw the grid. Returns true when a value changed.
pub fn show(ui: &mut egui::Ui, sheet: &mut DataSheet, editing: &mut Editing) -> bool {
    let mut changed = false;
    let table = &mut sheet.table;
    table.sync_grid_width();

    // Always keep a blank row and a blank dataset available to type into, the way
    // Prism's sheets grow as you fill them.
    let used = table.values.last_used_row().map_or(0, |r| r + 1);
    let visible_rows = (used + 8).max(24);
    if table.values.rows() < visible_rows {
        table.values.resize(visible_rows, table.values.cols());
    }

    let cols = table.values.cols();
    let x_width = table.x_width();
    let y_width = table.y_width().max(1);

    // Column headers: one per dataset, spanning its subcolumns.
    let header = |ui: &mut egui::Ui| {
        ui.horizontal(|ui| {
            ui.allocate_exact_size(Vec2::new(HEAD_W, ROW_H), Sense::hover());
            if x_width > 0 {
                let title = table.x.as_ref().map(|x| x.title.clone()).unwrap_or_default();
                let label = if title.is_empty() { "X".to_string() } else { title };
                ui.allocate_ui(Vec2::new(COL_W * x_width as f32, ROW_H), |ui| {
                    ui.add_sized(
                        Vec2::new(COL_W * x_width as f32, ROW_H),
                        egui::Label::new(RichText::new(label).strong()),
                    );
                });
            }
            for (i, y) in table.ys.iter().enumerate() {
                let label = if y.title.is_empty() {
                    format!("{}", (b'A' + (i % 26) as u8) as char)
                } else {
                    y.title.clone()
                };
                ui.add_sized(
                    Vec2::new(COL_W * y_width as f32, ROW_H),
                    egui::Label::new(RichText::new(label).strong()),
                );
            }
        });
    };
    header(ui);

    // Subcolumn headers, when a dataset spans more than one.
    if y_width > 1 {
        ui.horizontal(|ui| {
            ui.allocate_exact_size(Vec2::new(HEAD_W, 16.0), Sense::hover());
            if x_width > 0 {
                ui.allocate_exact_size(Vec2::new(COL_W * x_width as f32, 16.0), Sense::hover());
            }
            for _ in 0..table.ys.len() {
                for k in 0..y_width {
                    let name = subcolumn_name(table.data_format, k);
                    ui.add_sized(
                        Vec2::new(COL_W, 16.0),
                        egui::Label::new(RichText::new(name).small().weak()),
                    );
                }
            }
        });
    }
    ui.separator();

    let total_rows = table.values.rows();
    egui::ScrollArea::both().auto_shrink([false, false]).show_rows(
        ui,
        ROW_H,
        total_rows,
        |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        Vec2::new(HEAD_W, ROW_H),
                        egui::Label::new(RichText::new(format!("{}", row + 1)).weak()),
                    );
                    for col in 0..cols {
                        let is_x = col < x_width;
                        if cell_widget(ui, table, row, col, is_x, editing) {
                            changed = true;
                        }
                    }
                });
            }
        },
    );

    changed
}

fn subcolumn_name(format: SubcolumnFormat, k: usize) -> String {
    match format {
        SubcolumnFormat::Replicates => format!("{}", k + 1),
        SubcolumnFormat::MeanSd | SubcolumnFormat::MeanSdN => {
            ["Mean", "SD", "N"][k.min(2)].to_string()
        }
        SubcolumnFormat::MeanSem | SubcolumnFormat::MeanSemN => {
            ["Mean", "SEM", "N"][k.min(2)].to_string()
        }
        SubcolumnFormat::MeanCv | SubcolumnFormat::MeanCvN => {
            ["Mean", "%CV", "N"][k.min(2)].to_string()
        }
        SubcolumnFormat::LowHigh => ["Low", "High"][k.min(1)].to_string(),
        SubcolumnFormat::UpperLowerLimits => {
            ["Mean", "Lower", "Upper"][k.min(2)].to_string()
        }
        SubcolumnFormat::Single => String::new(),
    }
}

fn cell_widget(
    ui: &mut egui::Ui,
    table: &mut prism_doc::model::DataTable,
    row: usize,
    col: usize,
    is_x: bool,
    editing: &mut Editing,
) -> bool {
    let size = Vec2::new(COL_W, ROW_H);
    let mut changed = false;

    if editing.cell == Some((row, col)) {
        let resp = ui.add_sized(size, TextEdit::singleline(&mut editing.buffer));
        resp.request_focus();
        let commit = ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Tab))
            || resp.lost_focus();
        if commit {
            let text = editing.buffer.trim().to_string();
            let value = if text.is_empty() {
                Cell::Blank
            } else {
                match text.parse::<f64>() {
                    Ok(v) => Cell::Number(v),
                    Err(_) => Cell::Text(text),
                }
            };
            if table.values.get(row, col) != &value {
                table.values.set(row, col, value);
                changed = true;
            }
            editing.cell = None;
            editing.buffer.clear();
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            editing.cell = None;
            editing.buffer.clear();
        }
        return changed;
    }

    let cell = table.values.get(row, col);
    let text = match cell {
        Cell::Blank => String::new(),
        Cell::Number(v) => format_cell(*v, if is_x { table.x.as_ref() } else { None }),
        Cell::Text(s) => s.clone(),
    };
    let fill = if is_x { Color32::from_gray(246) } else { Color32::TRANSPARENT };
    let resp = ui.add_sized(
        size,
        egui::Button::new(RichText::new(text).monospace())
            .fill(fill)
            .frame(false)
            .sense(Sense::click()),
    );
    if resp.clicked() || resp.double_clicked() {
        editing.cell = Some((row, col));
        editing.buffer = match table.values.get(row, col) {
            Cell::Blank => String::new(),
            c => c.to_string(),
        };
    }
    changed
}

fn format_cell(v: f64, set: Option<&prism_doc::model::DataSet>) -> String {
    match set.and_then(|s| s.decimals) {
        Some(d) if d < 15 => format!("{:.*}", d as usize, v),
        _ => {
            if v == v.trunc() && v.abs() < 1e15 {
                format!("{}", v as i64)
            } else {
                format!("{v}")
            }
        }
    }
}
