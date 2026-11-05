//! Application state and window layout.

use std::path::PathBuf;

use egui::{Color32, RichText};
use prism_doc::graph::GraphSpec;
use prism_doc::model::{new_uid, DataSheet, DataSet, GraphKind, GraphSheet, TableFormat};
use prism_doc::Document;
use prism_render::Scene;

use crate::dialogs::{self, Dialog, DialogState};
use crate::grid::{self, Editing};
use crate::{inspector, paint};

/// Which sheet the centre pane is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Data(usize),
    Graph(usize),
    Info(usize),
    Analysis(usize),
}

pub struct App {
    doc: Document,
    path: Option<PathBuf>,
    dirty: bool,
    selection: Selection,
    editing: Editing,
    /// Cached scene for the selected graph, rebuilt when anything changes.
    scene: Option<Scene>,
    scene_for: Option<usize>,
    selected_series: Option<usize>,
    inspector_tab: inspector::Tab,
    dialogs: DialogState,
    zoom: f32,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let mut doc = Document::default();
        doc.data_sheets.push(starter_sheet());
        App {
            doc,
            path: None,
            dirty: false,
            selection: Selection::Data(0),
            editing: Editing::default(),
            scene: None,
            scene_for: None,
            selected_series: None,
            inspector_tab: inspector::Tab::default(),
            dialogs: DialogState::default(),
            zoom: 1.0,
            status: "New document".into(),
        }
    }
}

/// An empty XY sheet, the way Prism opens with one ready to type into.
fn starter_sheet() -> DataSheet {
    let mut sheet = DataSheet::new("Data 1", TableFormat::XY);
    sheet.table.ys = vec![DataSet::new("A"), DataSet::new("B")];
    sheet.table.sync_grid_width();
    sheet.table.values.resize(24, sheet.table.total_width());
    sheet
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut app = App::default();
        if let Some(p) = open {
            app.open_path(p);
        }
        app
    }

    fn open_path(&mut self, path: PathBuf) {
        match prism_doc::open(&path) {
            Ok(doc) => {
                let sheets = doc.data_sheets.len();
                let graphs = doc.graph_sheets.len();
                self.status = format!(
                    "{} — {sheets} data sheet{}, {graphs} graph{}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    if sheets == 1 { "" } else { "s" },
                    if graphs == 1 { "" } else { "s" }
                );
                self.doc = doc;
                self.path = Some(path);
                self.dirty = false;
                self.selection = if self.doc.data_sheets.is_empty() {
                    Selection::Graph(0)
                } else {
                    Selection::Data(0)
                };
                self.invalidate();
            }
            Err(e) => self.status = format!("Could not open {}: {e}", path.display()),
        }
    }

    fn save(&mut self, path: Option<PathBuf>) {
        let Some(path) = path.or_else(|| self.path.clone()) else {
            self.save_as();
            return;
        };
        match prism_doc::save(&self.doc, &path) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.dirty = false;
            }
            Err(e) => self.status = format!("Could not save: {e}"),
        }
    }

    fn save_as(&mut self) {
        let dialog = rfd::FileDialog::new()
            .add_filter("Prism document", &["prism"])
            .add_filter("Prism XML", &["pzfx"])
            .set_file_name("Untitled.prism");
        if let Some(path) = dialog.save_file() {
            self.save(Some(path));
        }
    }

    fn export_svg(&mut self) {
        let Selection::Graph(i) = self.selection else {
            self.status = "Select a graph to export".into();
            return;
        };
        let Some(scene) = self.rebuild_scene(i) else {
            self.status = "Nothing to export".into();
            return;
        };
        let name = format!("{}.svg", sanitize(&self.doc.graph_sheets[i].title));
        if let Some(path) =
            rfd::FileDialog::new().add_filter("SVG", &["svg"]).set_file_name(name).save_file()
        {
            match std::fs::write(&path, prism_render::to_svg(&scene)) {
                Ok(()) => self.status = format!("Exported {}", path.display()),
                Err(e) => self.status = format!("Could not export: {e}"),
            }
        }
    }

    fn export_csv(&mut self) {
        let Selection::Data(i) = self.selection else {
            self.status = "Select a data sheet to export".into();
            return;
        };
        let sheet = &self.doc.data_sheets[i];
        let name = format!("{}.csv", sanitize(&sheet.title));
        if let Some(path) =
            rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name(name).save_file()
        {
            let csv = csv_with_headers(sheet);
            match std::fs::write(&path, csv) {
                Ok(()) => self.status = format!("Exported {}", path.display()),
                Err(e) => self.status = format!("Could not export: {e}"),
            }
        }
    }

    /// Create a graph for the selected data sheet, as Prism's New Graph does.
    fn new_graph(&mut self) {
        let Selection::Data(i) = self.selection else {
            self.status = "Select a data sheet first".into();
            return;
        };
        let sheet = &self.doc.data_sheets[i];
        let mut spec = GraphSpec::default_for_table(&sheet.table);
        spec.source = Some(sheet.uid.clone());

        self.doc.graph_sheets.push(GraphSheet {
            uid: new_uid(),
            title: sheet.title.clone(),
            kind: GraphKind::MultiVariable,
            input_data_sets: sheet.table.ys.iter().map(|y| y.uid.clone()).collect(),
            data_sheet: Some(sheet.uid.clone()),
            spec,
            legacy_blob: None,
            raw_json: None,
        });
        self.selection = Selection::Graph(self.doc.graph_sheets.len() - 1);
        self.dirty = true;
        self.invalidate();
    }

    fn invalidate(&mut self) {
        self.scene = None;
        self.scene_for = None;
    }

    /// The table a graph draws from, by uid, falling back to the first sheet.
    fn table_for(&self, graph: usize) -> Option<&prism_doc::model::DataTable> {
        let g = self.doc.graph_sheets.get(graph)?;
        let want = g.spec.source.as_ref().or(g.data_sheet.as_ref());
        self.doc
            .data_sheets
            .iter()
            .find(|d| Some(&d.uid) == want)
            .or_else(|| {
                // Classic graphs name their datasets rather than their sheet.
                self.doc.data_sheets.iter().find(|d| {
                    g.input_data_sets.iter().any(|u| d.table.ys.iter().any(|y| &y.uid == u))
                })
            })
            .or_else(|| self.doc.data_sheets.first())
            .map(|d| &d.table)
    }

    fn rebuild_scene(&mut self, graph: usize) -> Option<Scene> {
        if self.scene_for == Some(graph) {
            if let Some(s) = &self.scene {
                return Some(s.clone());
            }
        }
        let table = self.table_for(graph)?.clone();
        let spec = self.doc.graph_sheets.get(graph)?.spec.clone();
        let scene = prism_render::render(&table, &spec);
        self.scene = Some(scene.clone());
        self.scene_for = Some(graph);
        Some(scene)
    }
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() { "graph".into() } else { s }
}

/// A data sheet as CSV with a header row, for use outside Prism.
fn csv_with_headers(sheet: &DataSheet) -> String {
    let t = &sheet.table;
    let mut head: Vec<String> = Vec::new();
    if t.x_width() > 0 {
        let base = t.x.as_ref().map(|x| x.title.clone()).unwrap_or_else(|| "X".into());
        let base = if base.is_empty() { "X".to_string() } else { base };
        for k in 0..t.x_width() {
            head.push(if k == 0 { base.clone() } else { format!("{base} ({})", k + 1) });
        }
    }
    for (i, y) in t.ys.iter().enumerate() {
        let base = if y.title.is_empty() { format!("Dataset {}", i + 1) } else { y.title.clone() };
        for k in 0..t.y_width() {
            head.push(if t.y_width() == 1 { base.clone() } else { format!("{base} ({})", k + 1) });
        }
    }

    let mut out = String::new();
    let quote = |s: &str| {
        if s.contains([',', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    out.push_str(&head.iter().map(|h| quote(h)).collect::<Vec<_>>().join(","));
    out.push('\n');

    let rows = t.values.last_used_row().map_or(0, |r| r + 1);
    for r in 0..rows {
        let cells: Vec<String> = (0..t.values.cols())
            .map(|c| match t.values.get(r, c) {
                prism_doc::Cell::Blank => String::new(),
                prism_doc::Cell::Number(v) => format!("{v}"),
                prism_doc::Cell::Text(s) => quote(s),
            })
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui.ctx());
        self.menu_bar(ui);
        self.toolbar(ui);
        self.navigator(ui);
        self.status_bar(ui);
        self.inspector_panel(ui);
        self.central(ui);
        self.dialogs(ui.ctx());

        // Opening a document by dropping it on the window.
        let dropped = ui.ctx().input(|i| i.raw.dropped_files.clone());
        if let Some(f) = dropped.first().map(|f| f.path().to_path_buf()) {
            self.open_path(f);
        }
    }
}

impl App {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (open, save, save_as, new_graph) = ctx.input_mut(|i| {
            use egui::{Key, Modifiers};
            (
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::G),
            )
        });
        if open {
            self.pick_and_open();
        }
        if save {
            self.save(None);
        }
        if save_as {
            self.save_as();
        }
        if new_graph {
            self.new_graph();
        }
    }

    fn pick_and_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Prism documents", &["prism", "prismt", "pzfx", "xml", "pzt"])
            .pick_file()
        {
            self.open_path(path);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::containers::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        let zoom = self.zoom;
                        *self = App::default();
                        self.zoom = zoom;
                        ui.close();
                    }
                    if ui.button("Open…").clicked() {
                        self.pick_and_open();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Save").clicked() {
                        self.save(None);
                        ui.close();
                    }
                    if ui.button("Save As…").clicked() {
                        self.save_as();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Export graph as SVG…").clicked() {
                        self.export_svg();
                        ui.close();
                    }
                    if ui.button("Export sheet as CSV…").clicked() {
                        self.export_csv();
                        ui.close();
                    }
                });
                ui.menu_button("Graph", |ui| {
                    if ui.button("New graph from this sheet").clicked() {
                        self.new_graph();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Change Graph Type…").clicked() {
                        self.dialogs.open = Some(Dialog::ChangeGraphType);
                        ui.close();
                    }
                    if ui.button("Format Graph…").clicked() {
                        self.dialogs.open = Some(Dialog::FormatGraph);
                        ui.close();
                    }
                    if ui.button("Format Axes…").clicked() {
                        self.dialogs.open = Some(Dialog::FormatAxes);
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Zoom in").clicked() {
                        self.zoom = (self.zoom * 1.25).min(8.0);
                    }
                    if ui.button("Zoom out").clicked() {
                        self.zoom = (self.zoom / 1.25).max(0.2);
                    }
                    if ui.button("Actual size").clicked() {
                        self.zoom = 1.0;
                    }
                });

                ui.separator();
                if let Some(p) = &self.path {
                    ui.label(
                        RichText::new(format!(
                            "{}{}",
                            p.file_name().unwrap_or_default().to_string_lossy(),
                            if self.dirty { " •" } else { "" }
                        ))
                        .weak(),
                    );
                }
            });
        });
    }

    /// The action strip under the menu, grouped the way Prism groups its toolbar.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                let on_data = matches!(self.selection, Selection::Data(_));
                let on_graph = matches!(self.selection, Selection::Graph(_));

                if ui
                    .add_enabled(on_data, egui::Button::new("New Graph"))
                    .on_hover_text("Build the graph this table calls for")
                    .clicked()
                {
                    self.new_graph();
                }
                ui.separator();
                if ui
                    .add_enabled(on_graph, egui::Button::new("Change…"))
                    .on_hover_text("Change the graph type")
                    .clicked()
                {
                    self.dialogs.open = Some(Dialog::ChangeGraphType);
                }
                if ui
                    .add_enabled(on_graph, egui::Button::new("Format Graph…"))
                    .clicked()
                {
                    self.dialogs.open = Some(Dialog::FormatGraph);
                }
                if ui
                    .add_enabled(on_graph, egui::Button::new("Format Axes…"))
                    .clicked()
                {
                    self.dialogs.open = Some(Dialog::FormatAxes);
                }
                ui.separator();
                if ui
                    .add_enabled(on_graph || on_data, egui::Button::new("Export…"))
                    .clicked()
                {
                    if on_graph {
                        self.export_svg();
                    } else {
                        self.export_csv();
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("+").clicked() {
                        self.zoom = (self.zoom * 1.25).min(8.0);
                    }
                    if ui.small_button("−").clicked() {
                        self.zoom = (self.zoom / 1.25).max(0.2);
                    }
                });
            });
        });
    }

    /// Host whichever modal dialog is open.
    fn dialogs(&mut self, ctx: &egui::Context) {
        let Selection::Graph(i) = self.selection else {
            self.dialogs.open = None;
            return;
        };
        if i >= self.doc.graph_sheets.len() {
            return;
        }
        let Some(table) = self.table_for(i).cloned() else { return };
        let changed = dialogs::show(
            ctx,
            &mut self.dialogs,
            &mut self.doc.graph_sheets[i].spec,
            &table,
        );
        if changed {
            self.dirty = true;
            self.invalidate();
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.0}%", self.zoom * 100.0)).small().weak());
                });
            });
        });
    }

    fn navigator(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("navigator").default_size(220.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let section = |ui: &mut egui::Ui, name: &str, count: usize| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{name} ({count})"))
                            .small()
                            .strong()
                            .color(Color32::from_gray(110)),
                    );
                };

                section(ui, "Data Tables", self.doc.data_sheets.len());
                for i in 0..self.doc.data_sheets.len() {
                    let title = self.doc.data_sheets[i].title.clone();
                    let label = if title.is_empty() { format!("Data {}", i + 1) } else { title };
                    if ui
                        .selectable_label(self.selection == Selection::Data(i), label)
                        .clicked()
                    {
                        self.selection = Selection::Data(i);
                        self.editing.cell = None;
                    }
                }

                section(ui, "Graphs", self.doc.graph_sheets.len());
                for i in 0..self.doc.graph_sheets.len() {
                    let g = &self.doc.graph_sheets[i];
                    let label = if g.title.is_empty() {
                        format!("Graph {}", i + 1)
                    } else {
                        g.title.clone()
                    };
                    let classic = g.kind == GraphKind::Classic;
                    let resp = ui.selectable_label(
                        self.selection == Selection::Graph(i),
                        if classic { format!("{label}  ·") } else { label },
                    );
                    if classic {
                        resp.clone().on_hover_text(
                            "Classic Prism graph. Its stored appearance is an undocumented \
                             binary blob, so it is shown with Prism's defaults for this table. \
                             The original blob is preserved when you save.",
                        );
                    }
                    if resp.clicked() {
                        self.selection = Selection::Graph(i);
                        self.invalidate();
                    }
                }

                if !self.doc.analyses.is_empty() {
                    section(ui, "Results", self.doc.analyses.len());
                    for i in 0..self.doc.analyses.len() {
                        let label = self.doc.analyses[i].title.clone();
                        if ui
                            .selectable_label(self.selection == Selection::Analysis(i), label)
                            .clicked()
                        {
                            self.selection = Selection::Analysis(i);
                        }
                    }
                }

                section(ui, "Layouts", 0);
                ui.label(RichText::new("  none").small().weak());

                if !self.doc.info_sheets.is_empty() {
                    section(ui, "Info", self.doc.info_sheets.len());
                    for i in 0..self.doc.info_sheets.len() {
                        let label = self.doc.info_sheets[i].title.clone();
                        if ui
                            .selectable_label(self.selection == Selection::Info(i), label)
                            .clicked()
                        {
                            self.selection = Selection::Info(i);
                        }
                    }
                }
            });
        });
    }

    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        let Selection::Graph(i) = self.selection else { return };
        if i >= self.doc.graph_sheets.len() {
            return;
        }
        egui::Panel::right("inspector").default_size(300.0).show(ui, |ui| {
            let mut selected = self.selected_series;
            let changed = inspector::show(
                ui,
                &mut self.inspector_tab,
                &mut self.doc.graph_sheets[i].spec,
                &mut selected,
            );
            if selected != self.selected_series {
                self.selected_series = selected;
            }
            if changed {
                self.dirty = true;
                self.invalidate();
            }
        });
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| match self.selection {
            Selection::Data(i) if i < self.doc.data_sheets.len() => {
                ui.horizontal(|ui| {
                    ui.heading(self.doc.data_sheets[i].title.clone());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("New graph").clicked() {
                            self.new_graph();
                        }
                    });
                });
                ui.separator();
                if grid::show(ui, &mut self.doc.data_sheets[i], &mut self.editing) {
                    self.dirty = true;
                    self.invalidate();
                }
            }
            Selection::Graph(i) if i < self.doc.graph_sheets.len() => {
                let scene = self.rebuild_scene(i);
                let highlight = self.selected_series;
                let zoom = self.zoom;
                egui::ScrollArea::both().show(ui, |ui| match scene {
                    Some(scene) => {
                        let size =
                            egui::Vec2::new(scene.size.0 * zoom, scene.size.1 * zoom);
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
                        paint::scene(ui.painter(), &scene, rect.min, zoom, highlight);
                    }
                    None => {
                        ui.label("This graph has no data sheet to draw from.");
                    }
                });
            }
            Selection::Analysis(i) if i < self.doc.analyses.len() => {
                let a = &self.doc.analyses[i];
                ui.heading(a.title.clone());
                ui.label(RichText::new(&a.analysis_class).small().weak());
                ui.separator();
                ui.label(
                    "Analyses are carried through unchanged. This build reads and writes the \
                     results Prism computed; it does not recompute them.",
                );
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.monospace(
                        serde_json::to_string_pretty(&a.parameters)
                            .unwrap_or_else(|_| "—".into()),
                    );
                });
            }
            Selection::Info(i) if i < self.doc.info_sheets.len() => {
                ui.heading(self.doc.info_sheets[i].title.clone());
                ui.separator();
                ui.label(self.doc.info_sheets[i].notes.clone());
            }
            _ => {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Open a Prism document to begin").weak());
                });
            }
        });
    }
}
