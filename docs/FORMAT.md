# GraphPad Prism file formats

Notes on how Prism lays out its documents, written so that this project can read
and write a researcher's own `.prism` and `.pzfx` files without Prism installed.
The goal is interoperability with one's own data — nothing here reproduces
Prism's functionality, and no attempt is made to work around licensing.

The description is drawn from **GraphPad Prism 11.0.2** (macOS): the XML schema
GraphPad ships inside the application, the 78 sample documents that ship with it,
and documents written by Prism 10.5 and 11.0. Observations are separated from
inferences; anything unresolved is marked as such.

## Evidence used

| Source | What it established |
| --- | --- |
| `Contents/Resources/PrismXMLSchema.xsd` (575 lines, shipped by GraphPad) | The complete `.pzfx` element and attribute vocabulary |
| 67 `.xml` + 8 `.pzfx` sample documents in `Contents/SharedSupport/SampleData` | Real-world use of every table kind |
| 11 `.prismt` templates in `SharedSupport` | The `.prism` container layout |
| Documents written by Prism 10.5.0.757 and 11.0.1.89 | Confirmation on files with actual data |

Prism itself is closed source. Nothing here is copied from or derived from
GraphPad source code, and this project contains none of their code or assets.

---

## 1. `.prism` and `.prismt` — the native format of Prism 10 and 11

A **ZIP archive**, with entries stored uncompressed. `.prismt` is the template
flavour: structurally identical, but carrying no cell values.

```text
document.json                       manifest
data/sheets/<sheet-uid>/sheet.json  one data sheet
data/sets/<set-uid>.json            one column (dataset)
data/tables/<table-uid>/content.json    row and column counts
data/tables/<table-uid>/data.csv        the cell values
data/tables/<table-uid>/data.dt         32 hex characters, purpose unresolved
graphs/<graph-uid>/sheet.json       graph identity, and settings for MV graphs
graphs/<graph-uid>/data.bin         classic graph appearance (opaque)
analyses/<uid>/sheet.json           analysis identity
analyses/<uid>/parameters.json      analysis inputs
analyses/<uid>/results.json         computed results
analyses/<uid>/result_sheets/*.json one per output table
info/<uid>/...                      info sheets
misc/*.bin                          used fonts and other side-cars
```

Every JSON object carries two reflection keys: `@class` names the C++/Obj-C class
that wrote it (`Document`, `DataSheet`, `XYDataTable`, `VariableDataSet`,
`FENGraphSheet`, `MVGraph`, `Axis`, `LinearAxisSegment`, …) and `$id` names its
JSON-Schema path (`sheets/data_sheet/data_sheet.schema.json`). The schema files
themselves are **not** shipped in the bundle.

### `document.json`

```jsonc
{
  "@class": "Document",
  "$id": "document.schema.json",
  "creationDate": "2025-04-16T00:00:00Z",
  "modificationDate": "2026-04-14T02:34:48Z",
  "createdBy":  { "name": "Prism", "user": "…", "version": "10.5.0.757", "platform": "Win" },
  "modifiedBy": { "name": "Prism", "user": "…", "version": "11.0.1.89",  "platform": "Win" },
  "formatVersion": "1-6-0",        // container version, not the Prism version
  "minFormatVersion": "1-4-0",
  "minPrismVersion": "10.3.0",
  "compatibility": [ { "formatVersion": "1-0-0", "action": "warningOpen" } ],
  "sheets": { "graphs": ["<uid>"], "analyses": ["<uid>"], "data": ["<uid>"] },
  "sheetAttributesMap": { "<uid>": { "title": "Cities" } },
  "uiSettings": { … },
  "customColors": ["#ffffffff", …]
}
```

Sheet **order** comes from the arrays under `sheets`; sheet **titles** come from
`sheetAttributesMap`, keyed by the same uids. Uids are upper-case hyphenated
UUIDs throughout.

### Data sheets

```jsonc
{
  "@class": "DataSheet",
  "uid": "49513155-…",
  "title": "Data 1",
  "table": {
    "@class": "XYDataTable",
    "uid": "E0C098F6-…",            // keys data/tables/<this>/ — NOT the sheet uid
    "format": "xy",                  // table kind
    "dataFormat": "y_replicates",    // subcolumn layout
    "replicatesCount": 3,
    "dataSets": ["<set-uid>", …],    // Y columns, in order
    "xDataSetFormat": "y_single",
    "xDataSet": "<set-uid>"          // X column, when the kind has one
  },
  "font": { "family": "Arial", "size": 10.0 },
  "flags": ["HIDE_ROW_TITLES", "ANALYSIS_VIEW"],
  "maxDataSets": 2048,
  "maxRow": 500000
}
```

`format` is one of `xy`, `column`, `twoway`, `contingency`, `survival`,
`partsofwhole`, `multivariable`, `nested`.

`dataFormat` describes how many physical subcolumns one Y dataset occupies:
`y_single` (1), `y_replicates` (`replicatesCount`), `y_sd`/`y_sem`/`y_cv` (2,
mean + spread), `y_sd_n`/`y_sem_n`/`y_cv_n` (3, mean + spread + N),
`y_low_high` (2), `y_upper_lower_limits` (3).

> The cell values are keyed by the **table** uid, not the sheet uid. Following
> the sheet uid finds nothing.

### Cell values — `data/tables/<table-uid>/`

`content.json` gives the dimensions:

```json
{ "$id": "sheets/data_sheet/data_table/data_table_storage.schema.json",
  "numberOfColumns": 44, "numberOfRows": 43, "version": 1 }
```

`data.csv` holds the values, **one CSV field per physical subcolumn** and one
record per row, in this order: the X subcolumns, then each Y dataset's
subcolumns in `dataSets` order. An XY table with 3 replicates and 2 Y datasets
therefore has `1 + 2x3 = 7` fields per record — which is what Prism writes.

Quoting is load-bearing: **a quoted field is text, a bare field is a number, and
an empty field is a blank cell.** Blank is "missing", never zero.

```csv
"Cairo, Egypt",,0.89224231184140379,2.35671923073764633
22.09,0.231472,0.072792,0.090051,0.188921,0.236264,0.048923
```

`data.dt` holds 32 lower-case hex characters. It is **not** the MD5, SHA-1 or
SHA-256 of `data.csv` (checked). `analyses/<uid>/parameters.dt` has the same
shape. Most likely a content token over the in-memory model rather than the
serialized bytes; **unresolved**. Prism rewrites it on save and accepts files
whose value does not match, so it is not an integrity gate.

### Columns — `data/sets/<set-uid>.json`

```jsonc
{
  "@class": "VariableDataSet",
  "uid": "055C7998-…",
  "format": "y_single",
  "title": "Porto, Portugal",
  "attributes": ["DS_ATTR_Y"],       // or DS_ATTR_X
  "replicates": [                     // one entry per physical subcolumn
    { "@class": "DriverReplicate", "valueType": "real",
      "commonAttributes": ["NUMERIC"], "decimalsLength": 3,
      "significantDigitsCount": 7, "firstRow": 0, "lastRow": 42 }
  ],
  "currentType": "continuous",        // continuous | categorical | label
  "isManual": true
}
```

### Graph sheets — two different representations

This is the single most consequential finding. Prism 11 writes graphs **two
different ways**, and which one you get depends on the graph's age and family.
Across the 20 graph sheets in the shipped templates plus real documents:

| `@class` | Count | Where the appearance lives |
| --- | --- | --- |
| `FENGraphSheet` | 13 | `graphs/<uid>/data.bin`, an opaque binary blob |
| `GraphSheet` with an `MVGraph` | 7 | Fully described as JSON in `sheet.json` |

A **classic** graph sheet carries almost nothing:

```json
{ "@class": "FENGraphSheet", "uid": "879387E7-…", "title": "Data 1",
  "floatingNotes": ["07A1829D-…"],
  "inputDataSets": ["20DFEC67-…", "2B4269A9-…"] }
```

Its `data.bin` begins with the magic **`PCFFGRA4`** followed by
`01 80 0c 00 00 00 90 01 90 01 …`. The same magic and header prefix appear in
every classic graph blob and in the `<Template>` payload of `.pzfx`, so the two
formats share one serializer.

A **multivariable** graph sheet (dendrogram, heatmap, bubble, clustered data)
instead spells its appearance out as JSON, using the classes `MVGraph`, `Axis`,
`LinearAxisSegment`, `CategoricalAxisSegment`, `CustomAxisTick`, `SymbolLegend`,
`CategoricalSymbolsLegend`, `MVCombinedLegend`, `GradientColorLegend`,
`CategoricalColormap`, `GradientColormap`, `SingleColormap`,
`SymbolSizeEvaluator`, `DendrogramGraphComponent`, and the figure settings types
`BubblesFigure::Settings`, `HeatmapFigure::Settings`,
`DendrogramFigure::Settings`, `ConfidenceEllipsesFigure::Settings`.

#### `PCFFGRA4` is not decoded here

The blob is a serialized object tree, and its layout is not documented. Decoding
it would be a sizeable project in its own right, and this repository does not
attempt it: classic graphs are instead rebuilt from their data table using
Prism's defaults, and the original blob is written back untouched so that Prism
still opens the file and shows the real graph.

The practical consequence for anyone using this library: **the appearance of a
classic graph does not round-trip**, while its data does.

### Analyses

`parameters.json` is a fully readable description of the analysis inputs:

```jsonc
{ "@class": "AnalysisParameters",
  "$id": "analyses/hclustering/parameters.schema.json",
  "analysisClass": "HIERARCHICAL_CLUSTERING",
  "content": {
    "data": { "inputType": "ROWS", "variables": [ { "col": { "idx": 1, "title": "Longitude", "uid": "…" } } ] },
    "calc": { "columnTransform": "STANDARDIZE", "distance": { "type": "EUCLIDIAN", "value": 2.0 },
              "linkage": "GROUP_AVERAGE" },
    "defineClusters": { "rows": { "method": "NUMBER_OF_CLUSTERS", "clusterCount": 5 } },
    "report": { "distanceMatrix": true, "copheneticCorrelation": true },
    "graph": { "plots": ["DENDROGRAM", "HEATMAP", "ELBOW_PLOT"] },
    "output": { "significantDigitCount": 4,
                "pValueStyle": { "type": "P_VALUE_STYLE_GP", "decimalDigitCount": 6 } }
  } }
```

`results.json` holds what Prism computed. Analysis **output** tables appear as
ordinary data sheets flagged `ANALYSIS_VIEW`.

---

## 2. `.pzfx` and `.xml` — the XML format

Specified by `PrismXMLSchema.xsd`, shipped inside the bundle. Root element
`GraphPadPrismFile` in namespace `http://graphpad.com/prism/Prism.htm`, with a
required `PrismXMLVersion` (samples use `5.00`).

```xml
<GraphPadPrismFile xmlns="http://graphpad.com/prism/Prism.htm" PrismXMLVersion="5.00">
  <Created><OriginalVersion CreatedByProgram="GraphPad Prism" CreatedByVersion="7.0.0.1"/></Created>
  <TableSequence><Ref ID="Table0" Selected="1"/></TableSequence>
  <Table ID="Table0" XFormat="numbers" YFormat="replicates" Replicates="3" TableType="XY">
    <Title>XY: Log-dose vs. response</Title>
    <XColumn Width="92" Decimals="2" Subcolumns="1">
      <Subcolumn><d>-10</d><d>-9</d><d/></Subcolumn>
    </XColumn>
    <YColumn Width="81" Decimals="0" Subcolumns="3">
      <Title>Control</Title>
      <Subcolumn><d>23</d><d>45</d></Subcolumn>
      …
    </YColumn>
  </Table>
</GraphPadPrismFile>
```

* `TableType`: `XY`, `OneWay`, `TwoWay`, `Contingency`, `Survival`,
  `PartsOfWhole`, `Legacy`, `Result`. `ExtTableType` adds `MultipleVariables`
  and `Nested`.
* `XFormat`: `none`, `text`, `numbers`, `error`, `series`, `date`,
  `startenddate`, `time`.
* `YFormat`: `replicates`, `SD`, `SE`, `CV`, `SDN`, `SEN`, `CVN`, `low-high`,
  `text`, `upper-lower-limits`.
* `<d/>` (self-closing) is a blank cell; `<d Excluded="1">` marks a value
  excluded from analysis.
* Titles and notes are mixed content with inline `<B> <I> <U> <SUP> <SUB>
  <Font> <BR/> <TextAlign> <LineStyles>` markup.

### Date and time X columns use *two* elements

A table whose `XFormat` is `date` or `time` writes **both** an `<XColumn>`
holding the elapsed number and an `<XAdvancedColumn>` holding the formatted text:

```xml
<XColumn Width="162" Subcolumns="1" Decimals="0">
  <Subcolumn><d>0</d><d>24</d><d>70</d></Subcolumn>
</XColumn>
<XAdvancedColumn Version="1" Width="162" Decimals="15" Subcolumns="1">
  <Subcolumn><d>13-Jul-2013</d><d>6-Aug-2013</d><d>21-Sep-2013</d></Subcolumn>
</XAdvancedColumn>
```

Deriving the X width from `XFormat` alone silently drops the second column; the
width has to be read from the file. (This repository had exactly that bug until
the shipped corpus caught it.)

### `.pzfx` carries no graphs

The schema has **no graph element at all**. A document's graphs are in a single
`<Template dt:dt="bin.base64">` element whose base64 payload is **zlib**
compressed; inflating it yields the same `PCFFGRA4` blob as `graphs/*/data.bin`
above.

```text
<Template> → base64 → zlib inflate → "PCFFGRA4" …
```

So `.pzfx` is a data interchange format. Round-tripping a `.pzfx` without
preserving that blob verbatim destroys every graph in the document.

---

## 3. What this repository implements

| Capability | Status |
| --- | --- |
| Read `.prism` / `.prismt` data, columns, titles, analyses | Complete, verified on every shipped sample |
| Write `.prism`, preserving unmodelled entries byte for byte | Complete |
| Read `.pzfx` / `.xml` data tables, including date/time X columns | Complete |
| Write `.pzfx`, preserving the `<Template>` graph blob | Complete |
| Multivariable graph settings from JSON | Partially modelled |
| Classic graph appearance from `PCFFGRA4` | **Not decoded.** Classic graphs are shown with Prism's defaults for their table, and the original blob is preserved on save |
| Running analyses | Not implemented; parameters and results pass through unchanged |
| `data.dt` construction | Unresolved; a content-derived token is written |

## 4. Open questions

1. How `data.dt` / `parameters.dt` are computed. Not MD5, SHA-1 or SHA-256 of
   the sibling file.
2. The `PCFFGRA4` tree grammar — node framing, class ids and field encoding.
3. Whether `Excluded="1"` has a `.prism` equivalent; nothing in the CSV or the
   dataset JSON appears to carry it.
4. The `compatibility` array's full `action` vocabulary; only `warningOpen` and
   `disable` have been observed.

