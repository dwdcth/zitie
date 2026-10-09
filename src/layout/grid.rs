//! 方块格纸型(米字格/田字格/回宫格/作文纸)的绘制:
//! 格子装饰线 + 墨迹居中的字符放置。

use super::ir::{IrOp, IrPath, StrokeSpec};
use super::model::{ColorConfig, GenerateParams, PaperKind};
use super::paginate::{CellContent, Geometry, PageRows};
use super::{laid_glyphs, Shaper};

const OUTER_WIDTH: f64 = 0.8;
const GUIDE_WIDTH: f64 = 0.5;
const DASH: (f64, f64) = (2.5, 2.0);
/// 字符在格内的留白比例(每侧)
const CELL_PADDING: f64 = 0.14;

pub fn paint_page(
    ops: &mut Vec<IrOp>,
    params: &GenerateParams,
    shaper: &mut Shaper<'_>,
    geo: &Geometry,
    page_idx: usize,
    rows: &PageRows,
) {
    let s = geo.cell_w;
    let colors = &params.colors;
    let ttb = params.direction.is_ttb(params.paper.is_ruled());
    // 每页的"线条数":横排 = 行数;竖排 = 列数(cols)
    let line_count = if ttb { geo.cols } else { geo.rows_of(page_idx) };

    for r in 0..line_count {
        if ttb {
            // 竖排:r = 从右数的第 r 列(geo.rtl 已镜像 x_of)
            let x = geo.x_of(r);
            for i in 0..geo.rows_of(page_idx) {
                let y = geo.cell_top_of(page_idx, i);
                paint_cell_frame(ops, params.paper, x, y, s, &colors.grid);
            }
            if let Some(column) = rows.get(r) {
                for cell in column {
                    // cell.col = 列内的垂直槽位
                    let y = geo.cell_top_of(page_idx, cell.col);
                    if let Some(op) =
                        place_in_cell(shaper, &cell.content, x, y, s, &params.font_id, colors)
                    {
                        ops.push(op);
                    }
                    if geo.pinyin_strip > 0.0 {
                        place_pinyin(
                            ops,
                            shaper,
                            &cell.content,
                            x,
                            y - geo.pinyin_strip,
                            geo,
                            params,
                        );
                    }
                }
            }
            continue;
        }
        // 横排(LTR/RTL)
        let y = geo.cell_top_of(page_idx, r);
        for c in 0..geo.cols {
            let x = geo.x_of(c);
            paint_cell_frame(ops, params.paper, x, y, s, &colors.grid);
        }
        // 字符与拼音
        if let Some(row) = rows.get(r) {
            for cell in row {
                let x = geo.x_of(cell.col);
                if let Some(op) =
                    place_in_cell(shaper, &cell.content, x, y, s, &params.font_id, colors)
                {
                    ops.push(op);
                }
                // 拼音条:单汉字 cluster 标注在格子正上方
                if geo.pinyin_strip > 0.0 {
                    place_pinyin(
                        ops,
                        shaper,
                        &cell.content,
                        x,
                        y - geo.pinyin_strip,
                        geo,
                        params,
                    );
                }
            }
        }
    }
}

/// 拼音标注:居中于格子上方的拼音条内,小字号实色。
/// 声调字母可能由多个字素构成(如 ǎ 的组合写法),复用 text_run_ops 逐字素出 op。
fn place_pinyin(
    ops: &mut Vec<IrOp>,
    shaper: &mut Shaper<'_>,
    content: &CellContent,
    cell_x: f64,
    strip_top: f64,
    geo: &Geometry,
    params: &GenerateParams,
) {
    let cluster = match content {
        CellContent::Empty => return,
        CellContent::Light(c) | CellContent::Solid(c) | CellContent::Demo(c) => c.as_str(),
    };
    let Some(py) = super::pinyin::pinyin_of(cluster) else {
        return;
    };
    let strip_h = geo.pinyin_strip;
    let size = strip_h * 0.62;
    let w = super::text_run_width(shaper, &py, size);
    let x = cell_x + (geo.cell_w - w) / 2.0;
    let baseline = strip_top + strip_h * 0.8;
    ops.extend(super::text_run_ops(
        shaper,
        &py,
        x,
        baseline,
        size,
        params.colors.solid,
        &params.font_id,
    ));
}

/// 格子装饰线。
fn paint_cell_frame(
    ops: &mut Vec<IrOp>,
    paper: PaperKind,
    x: f64,
    y: f64,
    s: f64,
    color: &super::model::Color,
) {
    let solid = StrokeSpec {
        color: *color,
        width: OUTER_WIDTH,
        dash: None,
    };
    ops.push(IrOp::Stroke {
        path: IrPath::rect(x, y, s, s),
        style: solid,
    });
    let dashed = StrokeSpec {
        color: *color,
        width: GUIDE_WIDTH,
        dash: Some((vec![DASH.0, DASH.1], 0.0)),
    };
    let mid = s / 2.0;
    match paper {
        PaperKind::MiZi => {
            // 十字中线 + 对角线(虚线)
            ops.push(IrOp::Stroke {
                path: IrPath::line(x + mid, y, x + mid, y + s),
                style: dashed.clone(),
            });
            ops.push(IrOp::Stroke {
                path: IrPath::line(x, y + mid, x + s, y + mid),
                style: dashed.clone(),
            });
            ops.push(IrOp::Stroke {
                path: IrPath::line(x, y, x + s, y + s),
                style: dashed.clone(),
            });
            ops.push(IrOp::Stroke {
                path: IrPath::line(x + s, y, x, y + s),
                style: dashed,
            });
        }
        PaperKind::TianZi => {
            ops.push(IrOp::Stroke {
                path: IrPath::line(x + mid, y, x + mid, y + s),
                style: dashed.clone(),
            });
            ops.push(IrOp::Stroke {
                path: IrPath::line(x, y + mid, x + s, y + mid),
                style: dashed,
            });
        }
        PaperKind::HuiGong => {
            // 内缩 1/4 的内框(实线细)
            let inset = s * 0.25;
            let inner = StrokeSpec {
                color: *color,
                width: GUIDE_WIDTH,
                dash: None,
            };
            ops.push(IrOp::Stroke {
                path: IrPath::rect(x + inset, y + inset, s - 2.0 * inset, s - 2.0 * inset),
                style: inner,
            });
        }
        PaperKind::ZuoWen => {} // 仅外框
        _ => unreachable!("线格纸型由 ruled 模块处理"),
    }
}

/// 在格子内放置字符:墨迹居中 + 等比缩放;字号上限为格子边长(避免标点被放大)。
fn place_in_cell(
    shaper: &mut Shaper<'_>,
    content: &CellContent,
    cell_x: f64,
    cell_y: f64,
    s: f64,
    font_id: &str,
    colors: &ColorConfig,
) -> Option<IrOp> {
    let cluster = match content {
        CellContent::Empty => return None,
        CellContent::Light(c) | CellContent::Solid(c) | CellContent::Demo(c) => c.as_str(),
    };
    let color = match content {
        CellContent::Light(_) => colors.light,
        CellContent::Solid(_) => colors.solid,
        CellContent::Demo(_) => colors.demo,
        CellContent::Empty => unreachable!(),
    };

    let shaped = shaper.shape(cluster);
    let target = s * (1.0 - 2.0 * CELL_PADDING);

    // 墨迹框(em 单位);无轮廓时退回 advance×em 框
    let (w_em, h_em, cx_em, cy_em) = match shaped.ink {
        Some(ink) if ink.width() > 1e-6 && ink.height() > 1e-6 => (
            ink.width(),
            ink.height(),
            (ink.x_min + ink.x_max) / 2.0,
            (ink.y_min + ink.y_max) / 2.0,
        ),
        _ => {
            let adv = super::advance_em(&shaped);
            let info = shaper.face_info();
            let asc = info.ascender / info.units_per_em;
            let desc = info.descender / info.units_per_em;
            (
                adv.max(1e-6),
                (asc - desc).max(1e-6),
                adv / 2.0,
                (asc + desc) / 2.0,
            )
        }
    };

    let size = (target / w_em.max(h_em)).min(s);
    let cell_cx = cell_x + s / 2.0;
    let cell_cy = cell_y + s / 2.0;
    // 墨迹中心对齐格子中心;字体的 y 向上,页面 y 向下
    let origin_x = cell_cx - cx_em * size;
    let baseline_y = cell_cy + cy_em * size;

    Some(IrOp::Glyphs {
        origin: (origin_x, baseline_y),
        size,
        font_id: font_id.to_string(),
        color,
        text: cluster.to_string(),
        glyphs: laid_glyphs(&shaped, cluster),
    })
}
