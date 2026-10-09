//! 排版中间表示:layout 引擎的输出、render 层的输入。
//! 坐标均为 pt,页面左上角为原点,y 向下(与 krilla surface 一致)。
//! glyph 的 advance/offset 已按 em 归一化(对齐 krilla 的 KrillaGlyph 约定)。

use crate::layout::model::Color;

#[derive(Debug, Clone)]
pub struct DocumentIr {
    pub width_pt: f64,
    pub height_pt: f64,
    pub pages: Vec<PageIr>,
}

impl DocumentIr {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

#[derive(Debug, Clone)]
pub struct PageIr {
    /// 从 1 开始
    pub index: usize,
    pub total: usize,
    pub ops: Vec<IrOp>,
}

#[derive(Debug, Clone)]
pub enum IrOp {
    Stroke {
        path: IrPath,
        style: StrokeSpec,
    },
    Glyphs {
        /// run 起点(笔位置,即首个 glyph 的基线原点)
        origin: (f64, f64),
        /// 字号 pt
        size: f64,
        font_id: String,
        color: Color,
        /// run 的原始文本(ToUnicode 映射用)
        text: String,
        glyphs: Vec<LaidGlyph>,
    },
}

#[derive(Debug, Clone)]
pub struct IrPath {
    pub els: Vec<PathEl>,
}

impl IrPath {
    pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Self {
        IrPath {
            els: vec![
                PathEl::Move(x, y),
                PathEl::Line(x + w, y),
                PathEl::Line(x + w, y + h),
                PathEl::Line(x, y + h),
                PathEl::Close,
            ],
        }
    }

    pub fn hline(x0: f64, x1: f64, y: f64) -> Self {
        IrPath {
            els: vec![PathEl::Move(x0, y), PathEl::Line(x1, y)],
        }
    }

    pub fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        IrPath {
            els: vec![PathEl::Move(x0, y0), PathEl::Line(x1, y1)],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.els.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PathEl {
    Move(f64, f64),
    Line(f64, f64),
    Close,
}

#[derive(Debug, Clone)]
pub struct StrokeSpec {
    pub color: Color,
    pub width: f64,
    /// 虚线 (段长, 间隙, 相位);None = 实线
    pub dash: Option<(Vec<f64>, f64)>,
}

#[derive(Debug, Clone)]
pub struct LaidGlyph {
    pub glyph_id: u16,
    /// em 归一化
    pub x_advance: f64,
    pub x_offset: f64,
    pub y_offset: f64,
    /// 在 `text` 中的字节区间(ToUnicode 用)
    pub range: std::ops::Range<usize>,
}
