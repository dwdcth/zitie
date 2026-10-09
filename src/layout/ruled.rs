//! 线格纸型(四线三格/单横线)的绘制:行线 + 基线对齐的字符放置。
//! 四线三格字号由字体 x-height 推导:小写 x 恰好填满中间格,
//! 上伸部自然进入上格、下伸部进入下格(以 ascender/descender 钳制不越线)。
//! 单横线:横线作为下伸部的底线(相当于四线三格的第 4 条线),
//! 基线上移一个下伸高度,字母悬于线上、下伸字母底部触线,不与横线交叉。
//! 格宽默认值按字体度量推导(以 m/W 的步进为准),保证字母间有间隙。

use super::ir::{IrOp, IrPath, StrokeSpec};
use super::model::{GenerateParams, PaperKind};
use super::paginate::{CellContent, Geometry, PageRows};
use super::{advance_em, laid_glyphs, Shaper};

const LINE_WIDTH: f64 = 0.5;
/// 单横线字号相对行距的比例(x 高 ≈ 0.34 行距,上伸部 ≈ 0.51,下伸部 ≈ 0.17)
const HENG_SIZE_RATIO: f64 = 0.68;
/// 字母最小间隙 pt
const LETTER_GAP: f64 = 3.0;

/// 该纸型下的字号(pt)。
pub fn font_size(shaper: &Shaper<'_>, params: &GenerateParams) -> f64 {
    match params.paper {
        PaperKind::SiXian => sixian_font_size(shaper, params.line_height()),
        PaperKind::HengXian => params.line_height() * HENG_SIZE_RATIO,
        _ => unreachable!("方块纸型无字号推导"),
    }
}

/// 格宽默认值:能容纳最宽常见字母(m/W)+ 间隙。
pub fn default_cell_width(shaper: &mut Shaper<'_>, size: f64) -> f64 {
    let adv = |s: &mut Shaper<'_>, c: &str| advance_em(&s.shape(c)) * size;
    let m = adv(shaper, "m");
    let w = adv(shaper, "W");
    (m.max(w) + LETTER_GAP).max(18.0)
}

pub fn paint_page(
    ops: &mut Vec<IrOp>,
    params: &GenerateParams,
    shaper: &mut Shaper<'_>,
    geo: &Geometry,
    page_idx: usize,
    rows: &PageRows,
) {
    let colors = &params.colors;
    let line_style = StrokeSpec {
        color: colors.grid,
        width: LINE_WIDTH,
        dash: None,
    };
    let x_right = geo.left + geo.content_w;
    let font_size = font_size(shaper, params);

    for r in 0..geo.rows_of(page_idx) {
        let y = geo.y_of(page_idx, r);
        let baseline_y = match params.paper {
            PaperKind::SiXian => {
                let h = params.line_height();
                for k in 0..=3 {
                    ops.push(IrOp::Stroke {
                        path: IrPath::hline(geo.left, x_right, y + k as f64 * h),
                        style: line_style.clone(),
                    });
                }
                y + 2.0 * h
            }
            PaperKind::HengXian => {
                // 横线在行带底部,作为下伸部的底线:基线上移一个下伸高度,
                // 字母整体悬于线上,下伸字母(q/y/g/p)底部恰好触线而不穿过
                let ly = y + params.line_height();
                ops.push(IrOp::Stroke {
                    path: IrPath::hline(geo.left, x_right, ly),
                    style: line_style.clone(),
                });
                let info = shaper.face_info();
                let desc_pt = (info.descender / info.units_per_em).abs() * font_size;
                ly - desc_pt
            }
            _ => unreachable!("方块纸型由 grid 模块处理"),
        };

        if let Some(row) = rows.get(r) {
            for cell in row {
                let cell_x = geo.x_of(cell.col);
                let cluster = match &cell.content {
                    CellContent::Empty => continue,
                    CellContent::Light(c) | CellContent::Solid(c) | CellContent::Demo(c) => c,
                };
                let color = match &cell.content {
                    CellContent::Light(_) => colors.light,
                    CellContent::Solid(_) => colors.solid,
                    CellContent::Demo(_) => colors.demo,
                    CellContent::Empty => continue,
                };
                let shaped = shaper.shape(cluster);
                // 字母在格内水平居中,基线对齐线格
                let adv = advance_em(&shaped) * font_size;
                let pen_x = cell_x + (geo.cell_w - adv) / 2.0;
                ops.push(IrOp::Glyphs {
                    origin: (pen_x, baseline_y),
                    size: font_size,
                    font_id: params.font_id.clone(),
                    color,
                    text: cluster.clone(),
                    glyphs: laid_glyphs(&shaped, cluster),
                });
            }
        }
    }
}

/// 四线三格字号:h = 中格高度;size = h / x_height,
/// 再用 ascender ≤ 2h、|descender| ≤ h 钳制,避免越出上下线。
fn sixian_font_size(shaper: &Shaper<'_>, h: f64) -> f64 {
    let info = shaper.face_info();
    let upm = info.units_per_em;
    let x_em = info
        .x_height
        .map(|v| v / upm)
        .filter(|v| *v > 0.1)
        .unwrap_or(0.5);
    let mut size = h / x_em;
    let asc_em = info.ascender / upm;
    if asc_em > 0.0 && asc_em * size > 2.0 * h {
        size = 2.0 * h / asc_em;
    }
    let desc_em = (info.descender / upm).abs();
    if desc_em > 0.0 && desc_em * size > h {
        size = h / desc_em;
    }
    size
}
