//! IR → PDF:把排版引擎的绘制指令翻译成 krilla 调用。
//! 渲染层不含任何整形/布局逻辑,glyph 几何全部来自 IR(em 归一化)。

use krilla::document::Document;
use krilla::geom::{Path, PathBuilder, Point};
use krilla::page::PageSettings;
use krilla::paint::{Fill, Paint, Stroke, StrokeDash};
use krilla::text::{Font, GlyphId, KrillaGlyph};

use crate::fonts::FontStore;
use crate::layout::ir::{DocumentIr, IrOp, IrPath, PathEl, StrokeSpec};
use crate::layout::model::Color;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("字体不存在或已失效,请重新选择字体")]
    FontMissing,
    #[error("PDF 生成失败:{0}")]
    Failed(String),
}

pub fn render_pdf(ir: &DocumentIr, store: &FontStore) -> Result<Vec<u8>, RenderError> {
    let mut doc = Document::new();
    for ir_page in &ir.pages {
        let page_settings = PageSettings::from_wh(ir.width_pt as f32, ir.height_pt as f32)
            .ok_or_else(|| RenderError::Failed("页面尺寸非法".into()))?;
        let mut kpage = doc.start_page_with(page_settings);
        let mut surface = kpage.surface();
        for op in &ir_page.ops {
            match op {
                IrOp::Stroke { path, style } => {
                    // fill/stroke 是 surface 的持久状态:画线前必须清掉残留的 fill,
                    // 否则后续格子矩形会被上一行的字形颜色填充
                    surface.set_fill(None);
                    surface.set_stroke(Some(to_stroke(style)));
                    if let Some(kp) = to_path(path) {
                        surface.draw_path(&kp);
                    }
                }
                IrOp::Glyphs { origin, size, font_id, color, text, glyphs } => {
                    let font: Font = store
                        .get(font_id)
                        .map(|e| e.krilla_font.clone())
                        .ok_or(RenderError::FontMissing)?;
                    surface.set_stroke(None);
                    surface.set_fill(Some(Fill {
                        paint: to_paint(*color),
                        ..Default::default()
                    }));
                    let kg: Vec<KrillaGlyph> = glyphs
                        .iter()
                        .map(|g| {
                            KrillaGlyph::new(
                                GlyphId::new(g.glyph_id as u32),
                                g.x_advance as f32,
                                g.x_offset as f32,
                                g.y_offset as f32,
                                0.0,
                                g.range.clone(),
                                None,
                            )
                        })
                        .collect();
                    surface.draw_glyphs(
                        Point::from_xy(origin.0 as f32, origin.1 as f32),
                        &kg,
                        font,
                        text.as_str(),
                        *size as f32,
                        false,
                    );
                }
            }
        }
        surface.finish();
        kpage.finish();
    }
    doc.finish().map_err(|e| RenderError::Failed(e.to_string()))
}

fn to_paint(c: Color) -> Paint {
    krilla::color::rgb::Color::new(c.0[0], c.0[1], c.0[2]).into()
}

fn to_stroke(s: &StrokeSpec) -> Stroke {
    Stroke {
        paint: to_paint(s.color),
        width: s.width as f32,
        dash: s.dash
            .as_ref()
            .map(|(array, offset)| StrokeDash {
                array: array.iter().map(|&v| v as f32).collect(),
                offset: *offset as f32,
            }),
        ..Default::default()
    }
}

fn to_path(p: &IrPath) -> Option<Path> {
    if p.els.is_empty() {
        return None;
    }
    let mut pb = PathBuilder::new();
    for el in &p.els {
        match *el {
            PathEl::Move(x, y) => pb.move_to(x as f32, y as f32),
            PathEl::Line(x, y) => pb.line_to(x as f32, y as f32),
            PathEl::Close => pb.close(),
        }
    }
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::model::*;

    fn font_store() -> FontStore {
        let base = format!("{}/assets/fonts", env!("CARGO_MANIFEST_DIR"));
        FontStore::load_builtin(std::path::Path::new(&base)).expect("内置字体应加载成功")
    }

    fn sample_ir(paper: PaperKind, mode: Mode, text: &str) -> DocumentIr {
        let store = font_store();
        let entry = store.get("lxgw-wenkai").expect("霞鹜文楷应存在");
        let p = GenerateParams {
            paper,
            mode,
            text: text.into(),
            header: HeaderConfig {
                title: Some("字帖练习".into()),
                fields: vec![HeaderField { label: "姓名".into(), value: String::new() }],
                page_number: true,
            },
            ..Default::default()
        };
        crate::layout::layout_document(&p, &entry).expect("布局应成功")
    }

    #[test]
    fn pdf_smoke_all_papers_modes() {
        let store = font_store();
        for paper in [PaperKind::MiZi, PaperKind::TianZi, PaperKind::HuiGong, PaperKind::ZuoWen, PaperKind::SiXian, PaperKind::HengXian] {
            for mode in [Mode::MiaoHong, Mode::ChaoXie, Mode::HunHe, Mode::ChunTie] {
                let ir = sample_ir(paper, mode, "永和九年 abc");
                let pdf = render_pdf(&ir, &store)
                    .unwrap_or_else(|e| panic!("{paper:?}/{mode:?} 渲染失败:{e}"));
                assert!(pdf.starts_with(b"%PDF"), "{paper:?}/{mode:?} 应为 PDF");
                let tail = &pdf[pdf.len().saturating_sub(64)..];
                assert!(tail.windows(5).any(|w| w == b"%%EOF"), "{paper:?}/{mode:?} 应有 EOF 标记");
                assert!(pdf.len() > 3_000, "{paper:?}/{mode:?} PDF 过小:{}", pdf.len());
            }
        }
    }

    #[test]
    fn render_is_deterministic() {
        let store = font_store();
        let ir = sample_ir(PaperKind::MiZi, Mode::HunHe, "永和");
        let a = render_pdf(&ir, &store).unwrap();
        let b = render_pdf(&ir, &store).unwrap();
        assert_eq!(a, b, "同 IR 两次渲染应字节一致(ETag/缓存的前提)");
    }

    /// 字体子集化:中文字帖 PDF 应远小于原字体(25MB)
    #[test]
    fn font_is_subset() {
        let store = font_store();
        let ir = sample_ir(PaperKind::MiZi, Mode::MiaoHong, "永和");
        let pdf = render_pdf(&ir, &store).unwrap();
        assert!(pdf.len() < 1_000_000, "PDF 应小于 1MB,实际 {} 字节", pdf.len());
    }
}
