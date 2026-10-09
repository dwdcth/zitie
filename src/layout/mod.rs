//! 排版引擎:纯计算,输入 `GenerateParams` + 字体,输出绘制指令 IR。
//! 与渲染层(krilla)完全解耦,便于单元测试。

pub mod graphemes;
pub mod grid;
pub mod header;
pub mod ir;
pub mod model;
pub mod paginate;
pub mod pinyin;
pub mod ruled;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

pub use model::{GenerateParams, PaperKind};

use crate::fonts::metrics::{self, ShapedCluster};
use crate::fonts::FontEntry;
use ir::{DocumentIr, IrOp, LaidGlyph};

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("{0}")]
    Invalid(String),
    #[error("内容过多:最多 {MAX_PAGES_STR} 页,请减少文字或练习次数")]
    TooManyPages,
}

const MAX_PAGES_STR: &str = "60";

/// 请求级整形器:按 cluster 记忆化,同一字重复出现时只整形一次。
pub struct Shaper<'a> {
    font: skrifa::FontRef<'a>,
    face: rustybuzz::Face<'a>,
    info: crate::fonts::metrics::FontFaceInfo,
    cache: HashMap<String, Arc<ShapedCluster>>,
}

impl<'a> Shaper<'a> {
    pub fn new(entry: &'a FontEntry) -> Shaper<'a> {
        let font = metrics::parse_font(&entry.bytes, 0)
            .expect("FontEntry 构建时已验证过可解析,此处不应失败");
        let face = rustybuzz::Face::from_slice(&entry.bytes, entry.face_index)
            .expect("FontEntry 构建时已验证过可解析,此处不应失败");
        let info = metrics::face_info(&font)
            .expect("FontEntry 构建时已验证过可解析,此处不应失败");
        Shaper { font, face, info, cache: HashMap::new() }
    }

    pub fn face_info(&self) -> crate::fonts::metrics::FontFaceInfo {
        self.info.clone()
    }

    pub fn shape(&mut self, cluster: &str) -> Arc<ShapedCluster> {
        if let Some(hit) = self.cache.get(cluster) {
            return hit.clone();
        }
        let shaped = Arc::new(metrics::shape_cluster(&self.face, &self.font, cluster));
        self.cache.insert(cluster.to_string(), shaped.clone());
        shaped
    }

    /// 文本中未被字体覆盖的字符(去重,最多列 n 个)。
    pub fn missing_chars(&self, text: &str, n: usize) -> Vec<char> {
        let mut missing: Vec<char> = Vec::new();
        for c in text.chars() {
            if c.is_whitespace() || c == '\u{feff}' {
                continue;
            }
            if !metrics::covers_char(&self.font, c) && !missing.contains(&c) {
                missing.push(c);
                if missing.len() >= n {
                    break;
                }
            }
        }
        missing
    }
}

/// 整形结果 → IR glyph 序列(补全 ToUnicode 字节区间)。
pub fn laid_glyphs(shaped: &ShapedCluster, text: &str) -> Vec<LaidGlyph> {
    shaped
        .glyphs
        .iter()
        .map(|g| {
            let start = g.byte_start.min(text.len());
            let end = shaped
                .glyphs
                .iter()
                .map(|x| x.byte_start)
                .filter(|&s| s > start)
                .min()
                .unwrap_or(text.len())
                .min(text.len());
            LaidGlyph {
                glyph_id: g.glyph_id,
                x_advance: g.x_advance,
                x_offset: g.x_offset,
                y_offset: g.y_offset,
                range: start..end.max(start),
            }
        })
        .collect()
}

/// PlacedGlyph advance 之和(em)。
pub fn advance_em(shaped: &ShapedCluster) -> f64 {
    shaped.glyphs.iter().map(|g| g.x_advance).sum()
}

/// 在基线上从 x 开始绘制一行文本(标题/字段/页码用,自然字距)。
pub fn text_run_ops(
    shaper: &mut Shaper<'_>,
    text: &str,
    x: f64,
    baseline_y: f64,
    size: f64,
    color: model::Color,
    font_id: &str,
) -> Vec<IrOp> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut ops = Vec::new();
    let mut pen = x;
    for g in text.graphemes(true) {
        let shaped = shaper.shape(g);
        let adv = advance_em(&shaped) * size;
        if !shaped.glyphs.is_empty() {
            ops.push(IrOp::Glyphs {
                origin: (pen, baseline_y),
                size,
                font_id: font_id.to_string(),
                color,
                text: g.to_string(),
                glyphs: laid_glyphs(&shaped, g),
            });
        }
        pen += adv;
    }
    ops
}

/// 测量一行文本宽度(pt)。
pub fn text_run_width(shaper: &mut Shaper<'_>, text: &str, size: f64) -> f64 {
    use unicode_segmentation::UnicodeSegmentation;
    text.graphemes(true)
        .map(|g| advance_em(&shaper.shape(g)) * size)
        .sum::<f64>()
}

/// 参数校验(生成前),错误消息面向用户。
pub fn validate(p: &GenerateParams) -> Result<(), LayoutError> {
    let err = |m: &str| LayoutError::Invalid(m.to_string());
    if p.trace_count < 1 || p.trace_count > model::MAX_REPEAT {
        return Err(err("每字描红格数须在 1–10 之间"));
    }
    if p.copy_count < 1 || p.copy_count > model::MAX_REPEAT {
        return Err(err("每字抄写格数须在 1–10 之间"));
    }
    let clusters = graphemes::cluster_count(&p.text);
    if clusters > model::MAX_CLUSTERS {
        return Err(err(&format!("文本过长(当前 {clusters} 个字,上限 {})", model::MAX_CLUSTERS)));
    }
    if let Some(t) = &p.header.title {
        if t.chars().count() > model::MAX_TITLE_CHARS {
            return Err(err(&format!("标题最多 {} 个字", model::MAX_TITLE_CHARS)));
        }
    }
    if p.header.fields.len() > model::MAX_FIELDS {
        return Err(err(&format!("自定义字段最多 {} 个", model::MAX_FIELDS)));
    }
    for f in &p.header.fields {
        if f.label.chars().count() > model::MAX_FIELD_LABEL {
            return Err(err(&format!("字段名最多 {} 个字", model::MAX_FIELD_LABEL)));
        }
        if f.value.chars().count() > model::MAX_FIELD_VALUE {
            return Err(err(&format!("字段值最多 {} 个字", model::MAX_FIELD_VALUE)));
        }
    }
    let m = &p.margins_mm;
    for (name, v) in [("上", m.top), ("右", m.right), ("下", m.bottom), ("左", m.left)] {
        if !(5.0..=40.0).contains(&v) {
            return Err(err(&format!("{name}边距须在 5–40mm 之间")));
        }
    }
    let range = |name: &str, v: f64, lo: f64, hi: f64| -> Result<(), LayoutError> {
        if !(lo..=hi).contains(&v) {
            Err(err(&format!("{name}须在 {lo:.0}–{hi:.0}pt 之间")))
        } else {
            Ok(())
        }
    };
    if let Some(v) = p.cell_size_pt {
        range("格子大小", v, 20.0, 100.0)?;
    }
    if let Some(v) = p.line_height_pt {
        range("行高", v, 10.0, 50.0)?;
    }
    if let Some(v) = p.cell_width_pt {
        range("格宽", v, 8.0, 100.0)?;
    }
    if let Some(v) = p.cols_per_row {
        if !(4..=40).contains(&v) {
            return Err(err("每行格数须在 4–40 之间"));
        }
    }
    range("格间距", p.col_gap_pt, 0.0, 30.0)?;
    range("行间距", p.row_gap_pt, 0.0, 40.0)?;
    Ok(())
}

/// 排版入口:参数 + 字体 → 文档 IR。
pub fn layout_document(
    params: &GenerateParams,
    entry: &FontEntry,
) -> Result<DocumentIr, LayoutError> {
    let mut shaper = Shaper::new(entry);
    let geometry = paginate::Geometry::new(params);
    let pages = paginate::paginate(params, &geometry);
    if pages.len() > model::MAX_PAGES {
        return Err(LayoutError::TooManyPages);
    }

    let total = pages.len();
    let mut doc = DocumentIr {
        width_pt: model::A4_W_PT,
        height_pt: model::A4_H_PT,
        pages: Vec::with_capacity(total),
    };

    for (page_idx, rows) in pages.iter().enumerate() {
        let mut ops = Vec::new();
        // 标题与自定义字段仅首页
        if page_idx == 0 {
            header::paint_header(&mut ops, params, &mut shaper, &geometry);
        }
        match params.paper {
            PaperKind::MiZi | PaperKind::TianZi | PaperKind::HuiGong | PaperKind::ZuoWen => {
                grid::paint_page(&mut ops, params, &mut shaper, &geometry, page_idx, rows);
            }
            PaperKind::SiXian | PaperKind::HengXian => {
                ruled::paint_page(&mut ops, params, &mut shaper, &geometry, page_idx, rows);
            }
        }
        if params.header.page_number {
            header::paint_page_number(&mut ops, params, &mut shaper, page_idx + 1, total);
        }
        doc.pages.push(ir::PageIr { index: page_idx + 1, total, ops });
    }

    Ok(doc)
}
