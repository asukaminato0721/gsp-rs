use crate::export::html::write_standalone_html;
use crate::format::{
    GspFile, ObjectGroup, RECORD_BACKGROUND_PALETTE_INDEX, RECORD_FONT_ENTRY, RECORD_PALETTE_ENTRY,
    Record, decode_indexed_path, decode_object_aux_u16, decode_object_aux_words,
    decode_object_group_header, decode_point_record, read_i16, read_u16, read_u32, record_name,
};
use miette::{IntoDiagnostic, Result, WrapErr, miette};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub fn compile_file_to_inspector(gsp_path: &Path, output_path: &Path) -> Result<()> {
    let data = fs::read(gsp_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read {}", gsp_path.display()))?;
    let file = crate::gsp::parse(&data).map_err(miette::Report::new)?;
    let htm = read_optional_text(&gsp_path.with_extension("htm"));
    let log = read_optional_text(&gsp_path.with_extension("log"));
    let html = render_inspector_html(gsp_path, &file, htm, log)?;
    write_standalone_html(output_path, &html).map_err(|error| miette!("{error}"))
}

fn read_optional_text(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct ByteSpan {
    offset: usize,
    length: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzedField {
    name: String,
    value: String,
    span: ByteSpan,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzedRecord {
    index: usize,
    record_type: u32,
    type_hex: String,
    name: String,
    span: ByteSpan,
    header_span: ByteSpan,
    payload_span: ByteSpan,
    padding_span: ByteSpan,
    page: Option<usize>,
    group_key: Option<String>,
    fields: Vec<AnalyzedField>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexedReference {
    record_index: usize,
    ordinal: usize,
    span: ByteSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzedGroup {
    key: String,
    page: usize,
    ordinal: usize,
    kind: String,
    kind_id: u16,
    class_id: u32,
    hidden: bool,
    span: ByteSpan,
    record_indices: Vec<usize>,
    references: Vec<IndexedReference>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyNode {
    key: String,
    ordinal: usize,
    kind: String,
    rank: usize,
    x: usize,
    y: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyEdge {
    source_key: String,
    target_key: String,
    record_index: usize,
    span: ByteSpan,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyGraph {
    page: usize,
    width: usize,
    height: usize,
    topology_complete: bool,
    unresolved_references: usize,
    nodes: Vec<DependencyNode>,
    edges: Vec<DependencyEdge>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectorDocument {
    file_name: String,
    magic: String,
    byte_length: usize,
    bytes_hex: String,
    page_count: usize,
    records: Vec<AnalyzedRecord>,
    groups: Vec<AnalyzedGroup>,
    dependency_graphs: Vec<DependencyGraph>,
    htm: Option<String>,
    log: Option<String>,
}

fn render_inspector_html(
    source_path: &Path,
    file: &GspFile,
    htm: Option<String>,
    log: Option<String>,
) -> Result<String> {
    let document = analyze_file(source_path, file, htm, log);
    let json = serde_json::to_string(&document)
        .into_diagnostic()
        .wrap_err("failed to serialize inspector data")?
        .replace('<', "\\u003c");
    Ok(INSPECTOR_HTML.replace("__GSP_INSPECTOR_DATA__", &json))
}

fn analyze_file(
    source_path: &Path,
    file: &GspFile,
    htm: Option<String>,
    log: Option<String>,
) -> InspectorDocument {
    let record_index_by_offset = file
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| (record.offset, index))
        .collect::<BTreeMap<_, _>>();

    let mut page_by_offset = BTreeMap::new();
    let mut group_by_offset = BTreeMap::new();
    let mut groups = Vec::new();
    for (page_index, page_file) in file.page_files().iter().enumerate() {
        let page = page_index + 1;
        for record in &page_file.records {
            page_by_offset.insert(record.offset, page);
        }
        for group in page_file.object_groups() {
            let key = format!("p{page}-g{}", group.ordinal);
            group_by_offset.insert(group.start_offset, key.clone());
            for record in &group.records {
                group_by_offset.insert(record.offset, key.clone());
            }
            groups.push(analyze_group(
                page,
                key,
                &group,
                &record_index_by_offset,
                &file.data,
            ));
        }
    }

    let records = file
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            analyze_record(
                index,
                record,
                file,
                page_by_offset.get(&record.offset).copied(),
                group_by_offset.get(&record.offset).cloned(),
            )
        })
        .collect();

    let mut bytes_hex = String::with_capacity(file.data.len() * 2);
    for byte in &file.data {
        let _ = write!(bytes_hex, "{byte:02x}");
    }

    let dependency_graphs = analyze_dependency_graphs(&groups);
    InspectorDocument {
        file_name: source_path
            .file_name()
            .unwrap_or(source_path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        magic: file.magic.clone(),
        byte_length: file.data.len(),
        bytes_hex,
        page_count: file.document_page_count(),
        records,
        groups,
        dependency_graphs,
        htm,
        log,
    }
}

fn analyze_dependency_graphs(groups: &[AnalyzedGroup]) -> Vec<DependencyGraph> {
    let mut groups_by_page = BTreeMap::<usize, Vec<&AnalyzedGroup>>::new();
    for group in groups {
        groups_by_page.entry(group.page).or_default().push(group);
    }
    groups_by_page
        .into_iter()
        .map(|(page, groups)| analyze_dependency_graph(page, &groups))
        .collect()
}

fn analyze_dependency_graph(page: usize, groups: &[&AnalyzedGroup]) -> DependencyGraph {
    let index_by_ordinal = groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.ordinal, index))
        .collect::<BTreeMap<_, _>>();
    let mut edges = Vec::new();
    let mut unresolved_references = 0usize;
    let mut indegrees = vec![0usize; groups.len()];
    let mut children = vec![Vec::new(); groups.len()];
    for (target_index, group) in groups.iter().enumerate() {
        for reference in &group.references {
            let Some(source_index) = index_by_ordinal.get(&reference.ordinal).copied() else {
                unresolved_references += 1;
                continue;
            };
            indegrees[target_index] += 1;
            children[source_index].push(target_index);
            edges.push(DependencyEdge {
                source_key: groups[source_index].key.clone(),
                target_key: group.key.clone(),
                record_index: reference.record_index,
                span: reference.span,
            });
        }
    }

    let mut ready = indegrees
        .iter()
        .enumerate()
        .filter(|(_, indegree)| **indegree == 0)
        .map(|(index, _)| (groups[index].ordinal, index))
        .collect::<BTreeSet<_>>();
    let mut ranks = vec![0usize; groups.len()];
    let mut processed = 0usize;
    while let Some((_, index)) = ready.pop_first() {
        processed += 1;
        for child in &children[index] {
            ranks[*child] = ranks[*child].max(ranks[index] + 1);
            indegrees[*child] -= 1;
            if indegrees[*child] == 0 {
                ready.insert((groups[*child].ordinal, *child));
            }
        }
    }
    let topology_complete = processed == groups.len();
    if !topology_complete {
        let fallback_rank = ranks.iter().copied().max().unwrap_or(0) + 1;
        for (index, indegree) in indegrees.iter().enumerate() {
            if *indegree > 0 {
                ranks[index] = fallback_rank;
            }
        }
    }

    let mut positions_in_rank = BTreeMap::<usize, usize>::new();
    let mut nodes = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let rank = ranks[index];
        let position = positions_in_rank.entry(rank).or_default();
        nodes.push(DependencyNode {
            key: group.key.clone(),
            ordinal: group.ordinal,
            kind: group.kind.clone(),
            rank,
            x: 30 + *position * 180,
            y: 30 + rank * 70,
        });
        *position += 1;
    }
    let widest_rank = positions_in_rank.values().copied().max().unwrap_or(1);
    let rank_count = positions_in_rank.keys().next_back().copied().unwrap_or(0) + 1;

    DependencyGraph {
        page,
        width: (widest_rank * 180 + 60).max(720),
        height: (rank_count * 70 + 60).max(400),
        topology_complete,
        unresolved_references,
        nodes,
        edges,
    }
}

fn analyze_group(
    page: usize,
    key: String,
    group: &ObjectGroup,
    record_index_by_offset: &BTreeMap<usize, usize>,
    data: &[u8],
) -> AnalyzedGroup {
    let mut record_indices = Vec::with_capacity(group.records.len() + 1);
    if let Some(index) = record_index_by_offset.get(&group.start_offset) {
        record_indices.push(*index);
    }
    record_indices.extend(
        group
            .records
            .iter()
            .filter_map(|record| record_index_by_offset.get(&record.offset).copied()),
    );

    let references = group
        .records
        .iter()
        .filter_map(|record| {
            if !matches!(record.record_type, 0x07d2 | 0x07d3) {
                return None;
            }
            let path = decode_indexed_path(record.payload(data))?;
            let record_index = *record_index_by_offset.get(&record.offset)?;
            Some(
                path.refs
                    .into_iter()
                    .enumerate()
                    .map(move |(index, ordinal)| IndexedReference {
                        record_index,
                        ordinal,
                        span: ByteSpan {
                            offset: record.payload_range.start + 4 + index * 4,
                            length: 4,
                        },
                    }),
            )
        })
        .flatten()
        .collect();

    AnalyzedGroup {
        key,
        page,
        ordinal: group.ordinal,
        kind: format!("{:?}", group.header.kind()),
        kind_id: group.header.kind_id(),
        class_id: group.header.class_id,
        hidden: group.header.is_hidden(),
        span: ByteSpan {
            offset: group.start_offset,
            length: group.end_offset.saturating_sub(group.start_offset),
        },
        record_indices,
        references,
    }
}

fn analyze_record(
    index: usize,
    record: &Record,
    file: &GspFile,
    page: Option<usize>,
    group_key: Option<String>,
) -> AnalyzedRecord {
    let payload_end = record.payload_range.end;
    let aligned_end = if payload_end.is_multiple_of(2) {
        payload_end
    } else {
        payload_end.saturating_add(1).min(file.data.len())
    };
    AnalyzedRecord {
        index,
        record_type: record.record_type,
        type_hex: format!("0x{:04x}", record.record_type),
        name: record_name(record.record_type).to_string(),
        span: ByteSpan {
            offset: record.offset,
            length: aligned_end.saturating_sub(record.offset),
        },
        header_span: ByteSpan {
            offset: record.offset,
            length: 8,
        },
        payload_span: ByteSpan {
            offset: record.payload_range.start,
            length: record.length as usize,
        },
        padding_span: ByteSpan {
            offset: payload_end,
            length: aligned_end.saturating_sub(payload_end),
        },
        page,
        group_key,
        fields: analyze_record_fields(record, &file.data),
    }
}

fn analyze_record_fields(record: &Record, data: &[u8]) -> Vec<AnalyzedField> {
    let mut fields = vec![
        field("record.length", record.length.to_string(), record.offset, 4),
        field(
            "record.type",
            format!("0x{:04x}", record.record_type),
            record.offset + 4,
            4,
        ),
    ];
    let payload = record.payload(data);
    let start = record.payload_range.start;
    match record.record_type {
        0x0384 => {
            if payload.len() >= 4 {
                fields.push(field(
                    "document.pageCount",
                    read_u16(payload, 2).to_string(),
                    start + 2,
                    2,
                ));
            }
            if payload.len() >= 22 {
                fields.push(field(
                    "document.canvasWidth",
                    read_u16(payload, 18).to_string(),
                    start + 18,
                    2,
                ));
                fields.push(field(
                    "document.canvasHeight",
                    read_u16(payload, 20).to_string(),
                    start + 20,
                    2,
                ));
            }
        }
        0x03e8 if payload.len() >= 8 => {
            fields.push(field(
                "document.displayOffsetX",
                read_i16(payload, 4).to_string(),
                start + 4,
                2,
            ));
            fields.push(field(
                "document.displayOffsetY",
                read_i16(payload, 6).to_string(),
                start + 6,
                2,
            ));
        }
        0x07d0 => {
            if let Some(header) = decode_object_group_header(payload) {
                fields.push(field(
                    "object.classId",
                    format!("{} (0x{:08x})", header.class_id, header.class_id),
                    start,
                    4,
                ));
                fields.push(field(
                    "object.kind",
                    format!("{:?} ({})", header.kind(), header.kind_id()),
                    start,
                    2,
                ));
                fields.push(field(
                    "object.flags",
                    format!("0x{:08x}", header.flags),
                    start + 4,
                    4,
                ));
                fields.push(field(
                    "object.styleA",
                    format!("0x{:08x}", header.style_a),
                    start + 8,
                    4,
                ));
                if payload.len() >= 16 {
                    fields.push(field(
                        "object.styleB",
                        format!("0x{:08x}", header.style_b),
                        start + 12,
                        4,
                    ));
                }
                if payload.len() >= 28 {
                    fields.push(field(
                        "object.styleC",
                        format!("0x{:08x}", header.style_c),
                        start + 16,
                        4,
                    ));
                }
            }
        }
        0x07d2 | 0x07d3 => {
            if let Some(path) = decode_indexed_path(payload) {
                fields.push(field("path.count", path.refs.len().to_string(), start, 4));
                fields.extend(path.refs.into_iter().enumerate().map(|(index, reference)| {
                    reference_field(
                        format!("path.refs[{index}]"),
                        reference,
                        start + 4 + index * 4,
                    )
                }));
            }
        }
        0x07d6 => {
            if let Some(words) = decode_object_aux_words(payload) {
                fields.extend(words.into_iter().enumerate().map(|(index, value)| {
                    field(
                        format!("aux.words[{index}]"),
                        format!("{} (0x{:04x})", value, value),
                        start + index * 2,
                        2,
                    )
                }));
            }
        }
        0x07d8 => {
            if let Some(value) = decode_object_aux_u16(payload) {
                fields.push(field(
                    "aux.value",
                    format!("{} (0x{:04x})", value, value),
                    start,
                    2,
                ));
            }
        }
        0x0899 => {
            if let Some(point) = decode_point_record(payload) {
                fields.push(field("point.x", point.x.to_string(), start, 8));
                fields.push(field("point.y", point.y.to_string(), start + 8, 8));
            }
        }
        RECORD_PALETTE_ENTRY if payload.len() >= 5 => {
            fields.push(field(
                "palette.index",
                read_u16(payload, 0).to_string(),
                start,
                2,
            ));
            fields.push(field("palette.red", payload[2].to_string(), start + 2, 1));
            fields.push(field("palette.green", payload[3].to_string(), start + 3, 1));
            fields.push(field("palette.blue", payload[4].to_string(), start + 4, 1));
        }
        RECORD_BACKGROUND_PALETTE_INDEX if payload.len() == 2 => {
            fields.push(field(
                "background.paletteIndex",
                read_u16(payload, 0).to_string(),
                start,
                2,
            ));
        }
        RECORD_FONT_ENTRY if payload.len() >= 8 => {
            fields.push(field(
                "font.index",
                read_u32(payload, 0).to_string(),
                start,
                4,
            ));
            fields.push(field(
                "font.pointSize",
                read_u16(payload, 6).to_string(),
                start + 6,
                2,
            ));
        }
        _ => {}
    }
    fields
}

fn field(
    name: impl Into<String>,
    value: impl Into<String>,
    offset: usize,
    length: usize,
) -> AnalyzedField {
    AnalyzedField {
        name: name.into(),
        value: value.into(),
        span: ByteSpan { offset, length },
        reference: None,
    }
}

fn reference_field(name: String, reference: usize, offset: usize) -> AnalyzedField {
    AnalyzedField {
        name,
        value: format!("object #{reference}"),
        span: ByteSpan { offset, length: 4 },
        reference: Some(reference),
    }
}

const INSPECTOR_HTML: &str = r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>GSP Binary Inspector</title>
<style>
*{box-sizing:border-box}
html,body{height:100%;margin:0;font-family:system-ui,sans-serif}
button,input{font:inherit}
.app{height:100%;display:grid;grid-template-rows:auto minmax(0,1fr)}
header{display:flex;gap:18px;align-items:center;padding:10px;border-bottom:1px solid}
h1{font-size:15px;margin:0;white-space:nowrap}
.stats,.small,.notice,.hex-help,.empty,.view-status{font-size:12px}
.stats,.mono,.hex-scroll,.source{font-family:ui-monospace,monospace}
.layout{min-height:0;display:grid;grid-template-columns:minmax(260px,22vw) minmax(480px,1fr) minmax(300px,28vw)}
.pane{min-width:0;min-height:0;border-right:1px solid}
.pane:last-child{border-right:0}
.sidebar,.details{display:grid}
.sidebar{grid-template-rows:auto auto minmax(0,1fr)}
.details{grid-template-rows:auto minmax(0,1fr)}
.tabs{display:flex;border-bottom:1px solid}
.tabs button{border:0;border-right:1px solid;padding:8px;background:none;cursor:pointer}
.tabs button.active,.list-item.active{font-weight:bold}
.filter{margin:8px;padding:6px;width:calc(100% - 16px)}
.list{overflow:auto;padding:0 6px 8px}
.list-item{display:block;width:100%;padding:6px;border:0;background:none;text-align:left;cursor:pointer}
.item-main{display:flex;gap:8px;align-items:baseline}
.item-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.badge{margin-left:auto;font-size:10px}
.main-pane,.hex-pane{display:grid;grid-template-rows:auto minmax(0,1fr)}
.view-status{margin-left:auto;padding:8px}
.hex-help{padding:8px;border-bottom:1px solid}
.hex-scroll{position:relative;overflow:auto;font-size:13px;line-height:24px}
.hex-spacer{position:relative;width:max-content;min-width:100%}
.hex-layer{position:absolute;top:0;left:0;min-width:100%}
.hex-row{position:absolute;left:0;height:24px;display:grid;grid-template-columns:92px repeat(16,30px) 150px;align-items:center;padding:0 10px}
.hex-byte{border:0;padding:2px 3px;background:none;font:inherit;cursor:pointer}
.hex-byte.selected,.hex-byte.field-selected{background:Highlight;color:HighlightText}
.ascii{padding-left:10px;white-space:pre}
.dag-view{overflow:auto}
.dag-edge{fill:none;stroke:currentColor}
.dag-node rect{fill:Canvas;stroke:currentColor}
.dag-node text{fill:CanvasText;font:12px ui-monospace,monospace}
.dag-node.selected rect{stroke-width:2}
.details-body{overflow:auto;padding:10px}
.details h2{font-size:15px;margin:0 0 8px}
.notice{line-height:1.5;margin-bottom:10px}
.kv{display:grid;grid-template-columns:minmax(90px,auto) 1fr;gap:5px 10px;font-size:12px;margin-bottom:12px}
.kv dd{margin:0;overflow-wrap:anywhere}
.section-title{font-size:11px;text-transform:uppercase;margin:14px 0 6px}
.field{display:grid;grid-template-columns:minmax(110px,1fr) minmax(100px,1fr) auto;gap:8px;width:100%;border:0;border-top:1px solid;padding:7px 4px;text-align:left;background:none;cursor:pointer}
.field-value{overflow-wrap:anywhere}
.link-button{padding:4px 7px;margin:2px;cursor:pointer}
.source{height:100%;overflow:auto;white-space:pre-wrap;word-break:break-word;margin:0;padding:10px;font-size:12px;line-height:1.5}
.empty{padding:14px}
@media(max-width:1050px){.layout{grid-template-columns:260px minmax(430px,1fr)}.details{position:fixed;right:0;top:46px;bottom:0;width:min(420px,92vw);border-left:1px solid;background:Canvas;z-index:1}}
</style>
</head>
<body>
<div class="app">
<header><h1 id="file-name">GSP Binary Inspector</h1><div class="stats" id="stats"></div></header>
<main class="layout">
  <aside class="pane sidebar">
    <div class="tabs"><button class="active" data-list-tab="records">Records</button><button data-list-tab="groups">Object groups</button></div>
    <input id="filter" class="filter" type="search" placeholder="type, name, offset…" aria-label="Filter">
    <div id="item-list" class="list"></div>
  </aside>
  <section class="pane main-pane">
    <div class="tabs"><button class="active" data-view-tab="hex">Hex</button><button data-view-tab="dag">DAG</button><span id="view-status" class="view-status"></span></div>
    <div id="hex-view" class="hex-pane">
      <div class="hex-help">点击字节定位 record；点击字段高亮其精确来源区间。未知 payload 不作推断。</div>
      <div id="hex-scroll" class="hex-scroll"><div id="hex-spacer" class="hex-spacer"><div id="hex-layer" class="hex-layer"></div></div></div>
    </div>
    <div id="dag-view" class="dag-view" hidden><svg id="dag-svg" aria-label="Object dependency graph"></svg></div>
  </section>
  <aside class="pane details">
    <div class="tabs"><button class="active" data-detail-tab="details">Details</button><button data-detail-tab="htm">.htm</button><button data-detail-tab="log">.log</button></div>
    <div id="details-body" class="details-body"></div>
  </aside>
</main>
</div>
<script id="inspector-data" type="application/json">__GSP_INSPECTOR_DATA__</script>
<script>
(()=>{
"use strict";
const data=JSON.parse(document.getElementById("inspector-data").textContent);
const $=id=>document.getElementById(id);
const listEl=$("item-list"),filterEl=$("filter"),hexView=$("hex-view"),hexScroll=$("hex-scroll"),hexSpacer=$("hex-spacer"),hexLayer=$("hex-layer"),dagView=$("dag-view"),dagSvg=$("dag-svg"),viewStatus=$("view-status"),detailsBody=$("details-body");
const rowHeight=24,bytesPerRow=16,totalRows=Math.ceil(data.byteLength/bytesPerRow),offsetDigits=Math.max(6,data.byteLength.toString(16).length);
const recordByIndex=new Map(data.records.map(record=>[record.index,record]));
const groupByKey=new Map(data.groups.map(group=>[group.key,group]));
const reverseRefs=new Map();
for(const group of data.groups){for(const ref of group.references){const key=`p${group.page}-g${ref.ordinal}`;if(!reverseRefs.has(key))reverseRefs.set(key,[]);reverseRefs.get(key).push(group.key)}}
const state={listTab:"records",viewTab:"hex",detailTab:"details",selectedRecord:null,selectedGroup:null,selection:{offset:0,length:0},fieldSelection:false};
function hexOffset(value){return "0x"+value.toString(16).padStart(offsetDigits,"0")}
function byteAt(offset){return parseInt(data.bytesHex.slice(offset*2,offset*2+2),16)}
function node(tag,className,text){const value=document.createElement(tag);if(className)value.className=className;if(text!==undefined)value.textContent=text;return value}
function svgNode(tag,attributes={}){const value=document.createElementNS("http://www.w3.org/2000/svg",tag);for(const [name,attribute] of Object.entries(attributes))value.setAttribute(name,String(attribute));return value}
function setActiveButtons(selector,attribute,value){for(const button of document.querySelectorAll(selector))button.classList.toggle("active",button.dataset[attribute]===value)}
function scrollToSpan(span){const row=Math.floor(span.offset/bytesPerRow);const top=row*rowHeight;if(top<hexScroll.scrollTop||top+rowHeight>hexScroll.scrollTop+hexScroll.clientHeight)hexScroll.scrollTop=Math.max(0,top-hexScroll.clientHeight/3)}
function selectSpan(span,fieldSelection=false,scroll=true){state.selection={offset:span.offset,length:Math.max(1,span.length)};state.fieldSelection=fieldSelection;if(scroll)scrollToSpan(span);if(state.viewTab==="hex")viewStatus.textContent=`${hexOffset(span.offset)} +${span.length}`;renderHex()}
function recordAt(offset){let low=0,high=data.records.length-1;while(low<=high){const mid=(low+high)>>1,record=data.records[mid];if(offset<record.span.offset)high=mid-1;else if(offset>=record.span.offset+record.span.length)low=mid+1;else return record}return null}
function selectRecord(index,scroll=true){const record=recordByIndex.get(index);if(!record)return;state.selectedRecord=index;state.selectedGroup=record.groupKey;state.detailTab="details";setActiveButtons("[data-detail-tab]","detailTab","details");selectSpan(record.span,false,scroll);renderList();renderDetails();if(state.viewTab==="dag")renderGraph()}
function selectGroup(key,scroll=true){const group=groupByKey.get(key);if(!group)return;state.selectedGroup=key;state.selectedRecord=null;state.detailTab="details";setActiveButtons("[data-detail-tab]","detailTab","details");selectSpan(group.span,false,scroll);renderList();renderDetails();if(state.viewTab==="dag")renderGraph()}
function setView(view){state.viewTab=view;hexView.hidden=view!=="hex";dagView.hidden=view!=="dag";setActiveButtons("[data-view-tab]","viewTab",view);if(view==="dag")renderGraph();else{viewStatus.textContent=`${hexOffset(state.selection.offset)} +${state.selection.length}`;renderHex()}}
function renderGraph(){
 const selected=state.selectedGroup?groupByKey.get(state.selectedGroup):null,page=selected?.page||1,graph=data.dependencyGraphs.find(candidate=>candidate.page===page);
 dagSvg.replaceChildren();
 if(!graph){viewStatus.textContent="No object groups";return}
 viewStatus.textContent=`page ${page} · ${graph.nodes.length} nodes · ${graph.edges.length} edges${graph.unresolvedReferences?` · ${graph.unresolvedReferences} unresolved`:""}${graph.topologyComplete?"":" · cycle"}`;
 dagSvg.setAttribute("width",graph.width);dagSvg.setAttribute("height",graph.height);dagSvg.setAttribute("viewBox",`0 0 ${graph.width} ${graph.height}`);
 const defs=svgNode("defs"),marker=svgNode("marker",{id:"dag-arrow",viewBox:"0 0 10 10",refX:9,refY:5,markerWidth:6,markerHeight:6,orient:"auto-start-reverse"}),arrow=svgNode("path",{d:"M 0 0 L 10 5 L 0 10 z",fill:"currentColor"});marker.append(arrow);defs.append(marker);dagSvg.append(defs);
 const nodeByKey=new Map(graph.nodes.map(item=>[item.key,item]));
 for(const edge of graph.edges){const source=nodeByKey.get(edge.sourceKey),target=nodeByKey.get(edge.targetKey);if(!source||!target)continue;const sx=source.x+75,sy=source.y+34,tx=target.x+75,ty=target.y,middle=(sy+ty)/2,bend=Math.max(sy,ty)+40,d=source===target?`M ${source.x+150} ${source.y+17} C ${source.x+195} ${source.y+17}, ${source.x+195} ${source.y-20}, ${tx} ${ty}`:target.rank<=source.rank?`M ${sx} ${sy} C ${sx} ${bend}, ${tx} ${bend}, ${tx} ${ty}`:`M ${sx} ${sy} C ${sx} ${middle}, ${tx} ${middle}, ${tx} ${ty}`,path=svgNode("path",{class:"dag-edge",d,"marker-end":"url(#dag-arrow)"}),link=svgNode("a",{href:"#","aria-label":`${source.ordinal} to ${target.ordinal}`}),title=svgNode("title");title.textContent=`#${source.ordinal} → #${target.ordinal} · ${hexOffset(edge.span.offset)}`;path.append(title);link.append(path);link.addEventListener("click",event=>{event.preventDefault();selectRecord(edge.recordIndex,false)});dagSvg.append(link)}
 for(const item of graph.nodes){const link=svgNode("a",{href:"#",class:"dag-node"+(item.key===state.selectedGroup?" selected":""),"aria-label":`Object ${item.ordinal} ${item.kind}`}),rect=svgNode("rect",{x:item.x,y:item.y,width:150,height:34}),label=svgNode("text",{x:item.x+8,y:item.y+21}),title=svgNode("title"),kind=item.kind.length>16?item.kind.slice(0,15)+"…":item.kind;label.textContent=`#${item.ordinal} ${kind}`;title.textContent=`#${item.ordinal} ${item.kind} · rank ${item.rank}`;link.append(rect,label,title);link.addEventListener("click",event=>{event.preventDefault();state.listTab="groups";setActiveButtons("[data-list-tab]","listTab","groups");selectGroup(item.key,false)});dagSvg.append(link)}
}
function renderHex(){
 const first=Math.max(0,Math.floor(hexScroll.scrollTop/rowHeight)-8),visible=Math.ceil(hexScroll.clientHeight/rowHeight)+16,last=Math.min(totalRows,first+visible);
 hexLayer.replaceChildren();
 for(let row=first;row<last;row++){
  const base=row*bytesPerRow,rowEl=node("div","hex-row");rowEl.style.top=`${row*rowHeight}px`;rowEl.append(node("span","hex-offset",hexOffset(base)));
  let ascii="";
  for(let column=0;column<bytesPerRow;column++){
   const offset=base+column;
   if(offset>=data.byteLength){rowEl.append(node("span"));ascii+=" ";continue}
   const value=byteAt(offset),button=node("button","hex-byte",value.toString(16).padStart(2,"0"));button.dataset.offset=String(offset);button.title=hexOffset(offset);
   if(offset>=state.selection.offset&&offset<state.selection.offset+state.selection.length)button.classList.add(state.fieldSelection?"field-selected":"selected");
   rowEl.append(button);ascii+=value>=32&&value<=126?String.fromCharCode(value):".";
  }
  rowEl.append(node("span","ascii",ascii));hexLayer.append(rowEl);
 }
}
function itemButton(title,subtitle,badge,onClick,active){const button=node("button","list-item"+(active?" active":""));const main=node("span","item-main");main.append(node("span","item-name",title));if(badge)main.append(node("span","badge",badge));button.append(main,node("span","small mono",subtitle));button.addEventListener("click",onClick);return button}
function renderList(){
 const query=filterEl.value.trim().toLowerCase();listEl.replaceChildren();
 if(state.listTab==="records"){
  for(const record of data.records){const haystack=`${record.index} ${record.typeHex} ${record.recordType} ${record.name} ${hexOffset(record.span.offset)} ${record.groupKey||""}`.toLowerCase();if(query&&!haystack.includes(query))continue;
   const page=record.page?`p${record.page} · `:"document · ",badge=record.groupKey||"";listEl.append(itemButton(`${record.typeHex} ${record.name}`,`${page}${hexOffset(record.span.offset)} · ${record.payloadSpan.length} payload bytes`,badge,()=>selectRecord(record.index),state.selectedRecord===record.index));
  }
 }else{
  for(const group of data.groups){const haystack=`${group.key} ${group.ordinal} ${group.kind} ${group.kindId} ${hexOffset(group.span.offset)}`.toLowerCase();if(query&&!haystack.includes(query))continue;
   listEl.append(itemButton(`#${group.ordinal} ${group.kind}`,`page ${group.page} · ${hexOffset(group.span.offset)} · ${group.span.length} bytes`,group.hidden?"hidden":"",()=>selectGroup(group.key),state.selectedGroup===group.key));
  }
 }
}
function definitionList(entries,container){const dl=node("dl","kv");for(const [key,value] of entries){dl.append(node("dt",null,key),node("dd","mono",String(value)))}container.append(dl)}
function section(title,container){container.append(node("div","section-title",title))}
function referenceButton(key,label){const button=node("button","link-button",label);button.addEventListener("click",()=>selectGroup(key));return button}
function renderRecordDetails(record){
 detailsBody.append(node("h2",null,`${record.typeHex} ${record.name}`));definitionList([["Index",record.index],["Offset",hexOffset(record.span.offset)],["Record bytes",record.span.length],["Payload",`${hexOffset(record.payloadSpan.offset)} +${record.payloadSpan.length}`],["Padding",record.paddingSpan.length],["Page",record.page||"document"],["Object group",record.groupKey||"—"]],detailsBody);
 if(record.groupKey){const jump=referenceButton(record.groupKey,"Open object group");detailsBody.append(jump)}
 section("Decoded fields",detailsBody);detailsBody.append(node("div","notice","Only fields backed by an exact layout in the Rust parser appear here."));
 for(const field of record.fields){const button=node("button","field");button.append(node("span","mono",field.name),node("span","field-value mono",field.value),node("span","small mono",`${hexOffset(field.span.offset)} +${field.span.length}`));button.addEventListener("click",()=>selectSpan(field.span,true));detailsBody.append(button)}
}
function renderGroupDetails(group){
 detailsBody.append(node("h2",null,`#${group.ordinal} ${group.kind}`));definitionList([["Key",group.key],["Page",group.page],["Kind ID",group.kindId],["Class ID",`0x${group.classId.toString(16).padStart(8,"0")}`],["Hidden",group.hidden],["Offset",hexOffset(group.span.offset)],["Bytes",group.span.length]],detailsBody);
 section("Records",detailsBody);const records=node("div");for(const index of group.recordIndices){const record=recordByIndex.get(index);const button=node("button","link-button",`${record.typeHex} ${record.name}`);button.addEventListener("click",()=>selectRecord(index));records.append(button)}detailsBody.append(records);
 section("Indexed references",detailsBody);if(!group.references.length)detailsBody.append(node("div","notice","No decoded 0x07d2/0x07d3 references."));for(const ref of group.references){const key=`p${group.page}-g${ref.ordinal}`,button=referenceButton(key,`#${ref.ordinal}`);if(!groupByKey.has(key)){button.disabled=true;button.title="Target group is not present in this page"}button.addEventListener("mouseenter",()=>selectSpan(ref.span,true,false));detailsBody.append(button)}
 section("Referenced by",detailsBody);const incoming=reverseRefs.get(group.key)||[];if(!incoming.length)detailsBody.append(node("div","notice","No decoded incoming references."));for(const key of incoming){const source=groupByKey.get(key);detailsBody.append(referenceButton(key,`#${source.ordinal} ${source.kind}`))}
}
function renderDetails(){
 detailsBody.className="details-body";detailsBody.replaceChildren();
 if(state.detailTab==="htm"||state.detailTab==="log"){const content=state.detailTab==="htm"?data.htm:data.log;if(content===null){detailsBody.append(node("div","empty",`No paired .${state.detailTab} file was found.`));return}const pre=node("pre","source");pre.textContent=content;detailsBody.className="";detailsBody.append(pre);return}
 const record=state.selectedRecord===null?null:recordByIndex.get(state.selectedRecord),group=state.selectedGroup?groupByKey.get(state.selectedGroup):null;if(record)renderRecordDetails(record);else if(group)renderGroupDetails(group);else detailsBody.append(node("div","empty","Select a record, object group, or byte."));
}
for(const button of document.querySelectorAll("[data-list-tab]"))button.addEventListener("click",()=>{state.listTab=button.dataset.listTab;setActiveButtons("[data-list-tab]","listTab",state.listTab);renderList()});
for(const button of document.querySelectorAll("[data-view-tab]"))button.addEventListener("click",()=>setView(button.dataset.viewTab));
for(const button of document.querySelectorAll("[data-detail-tab]"))button.addEventListener("click",()=>{state.detailTab=button.dataset.detailTab;setActiveButtons("[data-detail-tab]","detailTab",state.detailTab);renderDetails()});
filterEl.addEventListener("input",renderList);hexScroll.addEventListener("scroll",renderHex);window.addEventListener("resize",renderHex);
hexLayer.addEventListener("click",event=>{const target=event.target.closest("[data-offset]");if(!target)return;const offset=Number(target.dataset.offset),record=recordAt(offset);if(record){selectRecord(record.index,false);selectSpan({offset,length:1},true,false)}else selectSpan({offset,length:1},true,false)});
$("file-name").textContent=data.fileName;$("stats").textContent=`${data.magic} · ${data.byteLength.toLocaleString()} bytes · ${data.records.length.toLocaleString()} records · ${data.groups.length.toLocaleString()} groups · ${data.pageCount} page(s)`;
hexSpacer.style.height=`${totalRows*rowHeight}px`;hexSpacer.style.minWidth="740px";renderList();renderDetails();setView("hex");
})();
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::{analyze_file, compile_file_to_inspector, render_inspector_html};
    use crate::format::GspFile;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn push_record(data: &mut Vec<u8>, record_type: u32, payload: &[u8]) {
        data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        data.extend_from_slice(&record_type.to_le_bytes());
        data.extend_from_slice(payload);
        if !data.len().is_multiple_of(2) {
            data.push(0);
        }
    }

    fn fixture() -> GspFile {
        let mut data = b"GSP4".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&2_u32.to_le_bytes());
        header.extend_from_slice(&0x10000_u32.to_le_bytes());
        header.extend_from_slice(&0x0104_u32.to_le_bytes());
        push_record(&mut data, 0x07d0, &header);
        let mut path = Vec::new();
        path.extend_from_slice(&2_u32.to_le_bytes());
        path.extend_from_slice(&1_u32.to_le_bytes());
        path.extend_from_slice(&3_u32.to_le_bytes());
        push_record(&mut data, 0x07d2, &path);
        let mut point = Vec::new();
        point.extend_from_slice(&12.5_f64.to_le_bytes());
        point.extend_from_slice(&(-4.25_f64).to_le_bytes());
        push_record(&mut data, 0x0899, &point);
        push_record(&mut data, 0x07d7, &[]);
        GspFile::parse(&data).expect("fixture parses")
    }

    fn dag_fixture() -> GspFile {
        let mut data = b"GSP4".to_vec();
        for (kind, references) in [(0_u32, Vec::new()), (2_u32, vec![1_u32])] {
            let mut header = Vec::new();
            header.extend_from_slice(&kind.to_le_bytes());
            header.extend_from_slice(&0_u32.to_le_bytes());
            header.extend_from_slice(&0_u32.to_le_bytes());
            push_record(&mut data, 0x07d0, &header);
            if !references.is_empty() {
                let mut path = Vec::new();
                path.extend_from_slice(&(references.len() as u32).to_le_bytes());
                for reference in references {
                    path.extend_from_slice(&reference.to_le_bytes());
                }
                push_record(&mut data, 0x07d2, &path);
            }
            push_record(&mut data, 0x07d7, &[]);
        }
        GspFile::parse(&data).expect("DAG fixture parses")
    }

    #[test]
    fn analysis_preserves_spans_fields_groups_and_references() {
        let file = fixture();
        let analysis = analyze_file(Path::new("sample.gsp"), &file, None, None);
        assert_eq!(analysis.byte_length, file.data.len());
        assert_eq!(analysis.bytes_hex.len(), file.data.len() * 2);
        assert_eq!(analysis.groups.len(), 1);
        assert_eq!(analysis.groups[0].kind, "Segment");
        assert_eq!(analysis.groups[0].record_indices, vec![0, 1, 2, 3]);
        assert_eq!(
            analysis.groups[0]
                .references
                .iter()
                .map(|reference| reference.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(analysis.dependency_graphs.len(), 1);
        assert_eq!(analysis.dependency_graphs[0].edges.len(), 1);
        assert_eq!(analysis.dependency_graphs[0].unresolved_references, 1);
        assert!(!analysis.dependency_graphs[0].topology_complete);
        let path_record = &analysis.records[1];
        assert_eq!(path_record.group_key.as_deref(), Some("p1-g1"));
        assert!(
            path_record
                .fields
                .iter()
                .any(|field| field.name == "path.refs[1]" && field.reference == Some(3))
        );
        let point_record = &analysis.records[2];
        assert!(
            point_record
                .fields
                .iter()
                .any(|field| field.name == "point.y" && field.value == "-4.25")
        );
    }

    #[test]
    fn dependency_graph_layout_follows_exact_object_references() {
        let file = dag_fixture();
        let analysis = analyze_file(Path::new("dag.gsp"), &file, None, None);
        let graph = &analysis.dependency_graphs[0];
        assert!(graph.topology_complete);
        assert_eq!(graph.unresolved_references, 0);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source_key, "p1-g1");
        assert_eq!(graph.edges[0].target_key, "p1-g2");
        assert_eq!(graph.nodes[0].rank, 0);
        assert_eq!(graph.nodes[1].rank, 1);
        assert!(graph.nodes[0].y < graph.nodes[1].y);
    }

    #[test]
    fn standalone_html_embeds_sources_without_allowing_script_termination() {
        let file = fixture();
        let html = render_inspector_html(
            Path::new("sample.gsp"),
            &file,
            Some("<PARAM>\n</script><p>not markup</p>".to_string()),
            Some("payload log".to_string()),
        )
        .expect("inspector renders");
        assert!(html.contains("GSP Binary Inspector"));
        assert!(html.contains("data-view-tab=\"dag\""));
        assert!(html.contains("id=\"dag-svg\""));
        assert!(html.contains("\\u003c/script>"));
        assert!(!html.contains("</script><p>not markup</p>"));
        assert!(html.contains("payload log"));
        assert!(!html.contains("__GSP_INSPECTOR_DATA__"));
    }

    #[test]
    fn file_inspector_does_not_require_a_compilable_geometry_scene() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time moves forward")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gsp-rs-inspector-{unique}"));
        fs::create_dir_all(&root).expect("temporary directory is created");
        let gsp_path = root.join("unsupported.gsp");
        let output_path = root.join("unsupported.inspect.html");
        fs::write(&gsp_path, fixture().data).expect("fixture is written");
        fs::write(gsp_path.with_extension("htm"), "paired htm content").expect("htm is written");
        fs::write(gsp_path.with_extension("log"), "paired log content").expect("log is written");

        compile_file_to_inspector(&gsp_path, &output_path)
            .expect("binary inspection does not compile the geometry DAG");
        let html = fs::read_to_string(&output_path).expect("inspector output is readable");
        assert!(html.contains("paired htm content"));
        assert!(html.contains("paired log content"));
        assert!(html.contains("unsupported.gsp"));

        fs::remove_dir_all(root).expect("temporary directory is removed");
    }
}
