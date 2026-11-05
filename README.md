# rea-prism

Open, edit and plot your own GraphPad Prism data files on a machine that does not
have Prism on it. A small Rust desktop app that reads and writes both Prism
document formats, edits the data tables, and draws the usual graph types.

It exists for interoperability: getting at your own measurements when you are on
a machine without a Prism seat, scripting an export, or keeping a plot reproducible
alongside the rest of an analysis. It is not a substitute for Prism and does not
try to be — see [what it does not do](#what-it-does-not-do).

[`docs/FORMAT.md`](docs/FORMAT.md) describes how the documents are laid out.

![a dose–response curve and a column scatter, rendered by prism-render](docs/sample.svg)

## What works

* **Open** `.prism`, `.prismt`, `.pzfx` and `.xml` documents — the format is
  detected from the file's contents, not its extension.
* **Edit** data sheets in a spreadsheet view: all eight Prism table kinds,
  replicates, mean/SD/N subcolumns, text and date columns.
* **Graph** any sheet in ten families — XY, bar, column scatter, box and
  whisker, violin, survival, pie and donut, heatmap, bubble, dendrogram — with
  full control over axes, scales, ticks, symbols, lines, fills, error bars and
  legends.
* **Save** back to `.prism` or `.pzfx`. Entries the model does not cover are
  preserved byte for byte, so saving never silently discards part of a document.
* **Export** a graph to SVG, or a sheet to CSV.

## What it does not do

**No statistics.** This is the big one. Prism is a statistics package; this is
not. Analyses in an opened document are carried through with the parameters and
results Prism computed, but nothing is recalculated — no t-test, ANOVA,
regression or curve fit. The descriptive summaries behind the graphs (mean, SD,
SEM, quartiles) are computed for plotting only. **Do not report a number from
this app that you have not checked in Prism.**

**Classic graphs do not round-trip their appearance.** Prism stores the look of
XY, Column, Grouped, Survival and Parts-of-whole graphs in an undocumented binary
blob rather than in readable form. Opening such a document shows the graph
rebuilt from its data with sensible defaults — usually close, but not the
appearance you saved. The original blob is preserved and written back unchanged,
so nothing is destroyed and Prism still opens the file and shows the real graph.
Only the newer multivariable graphs (dendrogram, heatmap, bubble) store their
settings readably.

## Install

Needs a Rust toolchain (1.82 or newer). If you do not have one:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then:

```sh
git clone https://github.com/conmmul/rea-prism
cd rea-prism
cargo build --release
```

The result is a single self-contained binary at `target/release/rea-prism`.
Copy it anywhere on your `PATH`.

### A macOS app bundle

```sh
./scripts/bundle-macos.sh
```

writes `dist/Prism.app`, which you can drag to `/Applications` and open from
Finder. It is unsigned, so the first launch needs right-click → Open.

## Use

```sh
rea-prism                      # start with an empty XY sheet
rea-prism experiment.prism     # open a document
```

You can also drop a document onto the window.

| | |
| --- | --- |
| `Cmd-O` | Open |
| `Cmd-S` | Save |
| `Cmd-Shift-S` | Save As |
| `Cmd-G` | New graph from the selected sheet |

Pick a sheet in the navigator on the left. Data sheets open in the grid; click a
cell to type into it. **New graph** builds the graph Prism would make by default
for that table, and the format panel on the right changes everything about it.

## As a library

```toml
[dependencies]
prism-doc = { git = "https://github.com/conmmul/rea-prism" }
prism-render = { git = "https://github.com/conmmul/rea-prism" }
```

```rust
use prism_doc::graph::GraphSpec;

let doc = prism_doc::open("experiment.prism")?;
let sheet = &doc.data_sheets[0];

for (i, y) in sheet.table.ys.iter().enumerate() {
    let values: Vec<f64> = sheet.table.values
        .numeric_column(sheet.table.y_offset(i))
        .collect();
    println!("{}: n={}", y.title, values.len());
}

let spec = GraphSpec::default_for_table(&sheet.table);
std::fs::write("graph.svg", prism_render::to_svg(&prism_render::render(&sheet.table, &spec)))?;

prism_doc::save(&doc, "copy.pzfx")?;   // format chosen from the extension
# Ok::<(), prism_doc::Error>(())
```

Headless SVG export without the GUI:

```sh
cargo run -p prism-render --example export -- experiment.prism ./out xy
```

## Layout

| Crate | |
| --- | --- |
| `prism-doc` | Document model, `.prism` and `.pzfx` readers and writers |
| `prism-render` | Scales, layout, the ten plot families, SVG output |
| `prism-gui` | The `rea-prism` application |

`prism-render` emits a backend-independent list of primitives, so the GUI and the
SVG exporter draw identical geometry.

## Tests

```sh
cargo test --workspace
```

Beyond unit tests, the suite runs against the 78 sample documents inside a local
`Prism 11.app`: every one is parsed, round-tripped through both writers with its
cell values compared, and rendered in all ten graph families with every
coordinate checked. Those tests skip themselves when Prism is not installed. They
have already caught three real bugs: a dropped date subcolumn, an index underflow
in the dendrogram, and row titles being silently discarded on read.

## Licence

MIT or Apache-2.0, at your option.

Not affiliated with, endorsed by, or derived from GraphPad Software. "Prism" and
"GraphPad" are their trademarks, used here only to say which files this reads.

This project contains no GraphPad code, assets or sample documents. It exists so
that people can work with data files they already own, and it neither replaces
Prism nor circumvents any part of it. If you have a Prism licence, keep using
Prism for anything you intend to publish.
