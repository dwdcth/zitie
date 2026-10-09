//! 分页:cluster → 练习单元 → 行 → 页 的流式布局。

use super::graphemes::Token;
use super::model::{GenerateParams, Mode, PaperKind};

/// 一个格子的内容。
#[derive(Debug, Clone, PartialEq)]
pub enum CellContent {
    /// 描红(浅色)
    Light(String),
    /// 实色(纯字帖)
    Solid(String),
    /// 行首示范
    Demo(String),
    /// 空格
    Empty,
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub col: usize,
    pub content: CellContent,
}

pub type Row = Vec<Cell>;
pub type PageRows = Vec<Row>;

/// 页面几何(全部 pt,top-left 原点)。
pub struct Geometry {
    pub left: f64,
    pub top_first: f64,
    pub top_rest: f64,
    pub content_w: f64,
    pub cols: usize,
    pub rows_first: usize,
    pub rows_rest: usize,
    pub cell_w: f64,
    pub row_h: f64,
    pub col_gap: f64,
    pub row_gap: f64,
}

impl Geometry {
    pub fn new(p: &GenerateParams) -> Geometry {
        let left = p.margins_mm.left * super::model::MM_TO_PT;
        let right = p.margins_mm.right * super::model::MM_TO_PT;
        let top = p.margins_mm.top * super::model::MM_TO_PT;
        let bottom = p.margins_mm.bottom * super::model::MM_TO_PT;
        let content_w = super::model::A4_W_PT - left - right;

        let has_title = p.header.title.as_deref().is_some_and(|t| !t.trim().is_empty());
        let has_fields = p.header.fields.iter().any(|f| !f.label.trim().is_empty());
        let header_h = (if has_title { 30.0 } else { 0.0 }) + (if has_fields { 20.0 } else { 0.0 })
            + if has_title || has_fields { 8.0 } else { 0.0 };
        let pn_h = if p.header.page_number { 14.0 } else { 0.0 };

        let (cell_w, row_h) = match p.paper {
            PaperKind::SiXian => (p.cell_width(), p.line_height() * 3.0),
            PaperKind::HengXian => (p.cell_width(), p.line_height()),
            _ => (p.cell_size(), p.cell_size()),
        };

        let ch_first = super::model::A4_H_PT - top - header_h - bottom - pn_h;
        let ch_rest = super::model::A4_H_PT - top - bottom - pn_h;

        let fit = |avail: f64| -> usize {
            if row_h <= 0.0 {
                return 1;
            }
            (((avail + p.row_gap_pt) / (row_h + p.row_gap_pt)).floor() as usize).max(1)
        };

        Geometry {
            left,
            top_first: top + header_h,
            top_rest: top,
            content_w,
            cols: p
                .cols_per_row
                .map(|c| c as usize)
                .unwrap_or_else(|| {
                    if cell_w <= 0.0 {
                        1
                    } else {
                        (((content_w + p.col_gap_pt) / (cell_w + p.col_gap_pt)).floor() as usize)
                            .max(1)
                    }
                }),
            rows_first: fit(ch_first.max(row_h)),
            rows_rest: fit(ch_rest.max(row_h)),
            cell_w,
            row_h,
            col_gap: p.col_gap_pt,
            row_gap: p.row_gap_pt,
        }
    }

    /// 第 page_idx 页(0 起)的内容区顶部 y。
    pub fn top_of(&self, page_idx: usize) -> f64 {
        if page_idx == 0 {
            self.top_first
        } else {
            self.top_rest
        }
    }

    pub fn rows_of(&self, page_idx: usize) -> usize {
        if page_idx == 0 {
            self.rows_first
        } else {
            self.rows_rest
        }
    }

    /// 第 col 列格子的左边界 x。
    pub fn x_of(&self, col: usize) -> f64 {
        self.left + col as f64 * (self.cell_w + self.col_gap)
    }

    /// 第 row 行(页内)的顶部 y。
    pub fn y_of(&self, page_idx: usize, row: usize) -> f64 {
        self.top_of(page_idx) + row as f64 * (self.row_h + self.row_gap)
    }
}

/// 一个 cluster 按模式展开为练习单元序列。
fn expand(cluster: &str, p: &GenerateParams) -> Vec<CellContent> {
    let trace = p.trace_count as usize;
    let copy = p.copy_count as usize;
    match p.mode {
        Mode::MiaoHong => vec![CellContent::Light(cluster.to_string()); trace],
        Mode::ChaoXie => vec![CellContent::Empty; copy],
        Mode::HunHe => {
            let mut v = vec![CellContent::Light(cluster.to_string()); trace];
            v.extend(std::iter::repeat_n(CellContent::Empty, copy));
            v
        }
        Mode::ChunTie => vec![CellContent::Solid(cluster.to_string())],
    }
}

/// 流式分页。返回每页的行(行内格子按 col 顺序,可能不满一行)。
#[allow(unused_assignments)] // flush_row 宏末次写 demo_pending 无后续读取
pub fn paginate(p: &GenerateParams, geo: &Geometry) -> Vec<PageRows> {
    let continuous = p.paper.is_continuous();
    let want_demo = p.mode == Mode::ChaoXie && p.row_leading_demo && !continuous;
    let cols = geo.cols;

    let mut pages: Vec<PageRows> = vec![Vec::new()];
    let mut cur_row: Row = Vec::new();
    let mut demo_pending = true; // 当前行是否还未放过示范字

    macro_rules! flush_row {
        ($force:expr) => {{
            let force = $force;
            if !cur_row.is_empty() || force {
                let cap = geo.rows_of(pages.len() - 1);
                if pages.last().map_or(0, |pg| pg.len()) >= cap {
                    pages.push(Vec::new());
                }
                pages.last_mut().unwrap().push(std::mem::take(&mut cur_row));
                demo_pending = true;
            }
        }};
    }

    let tokens = super::graphemes::tokenize(&p.text);
    for token in tokens {
        match token {
            Token::Newline => flush_row!(true),
            Token::Space => {
                // 空格占一个空格:作文纸占位,练习纸作为词间分隔/休整格
                if cur_row.len() >= cols {
                    flush_row!(false);
                }
                cur_row.push(Cell { col: cur_row.len(), content: CellContent::Empty });
            }
            Token::Cluster(c) => {
                // 作文纸连续填格
                let units: Vec<CellContent> = if continuous {
                    vec![match p.mode {
                        Mode::MiaoHong | Mode::HunHe => CellContent::Light(c.clone()),
                        Mode::ChaoXie => CellContent::Empty,
                        Mode::ChunTie => CellContent::Solid(c.clone()),
                    }]
                } else {
                    expand(&c, p)
                };
                for unit in units {
                    if cur_row.len() >= cols {
                        flush_row!(false);
                    }
                    // 行首示范:该行第一个单元属于某 cluster 时补一个示范格(不占练习配额)
                    if want_demo && demo_pending && cur_row.is_empty() {
                        cur_row.push(Cell { col: 0, content: CellContent::Demo(c.clone()) });
                        demo_pending = false;
                    }
                    cur_row.push(Cell { col: cur_row.len(), content: unit });
                }
            }
        }
    }
    flush_row!(false);

    // 空文本(或纯空白):一页空白练习纸
    if pages.len() == 1 && pages[0].is_empty() {
        pages[0] = Vec::new();
    }
    pages
}
