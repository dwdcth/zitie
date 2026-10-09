//! 页眉(标题 + 自定义字段,仅首页)与页码(所有页)的绘制。

use super::ir::{IrOp, IrPath, StrokeSpec};
use super::model::GenerateParams;
use super::paginate::Geometry;
use super::{text_run_ops, text_run_width, Shaper};

const TITLE_SIZE: f64 = 18.0;
const FIELD_SIZE: f64 = 11.0;
const PAGE_NO_SIZE: f64 = 9.0;
/// 空字段值的下划线长度(pt)
const BLANK_LINE_LEN: f64 = 64.0;

/// 标题(居中)与字段行(左侧起均匀分布),仅首页调用。
pub fn paint_header(
    ops: &mut Vec<IrOp>,
    params: &GenerateParams,
    shaper: &mut Shaper<'_>,
    geo: &Geometry,
) {
    let colors = &params.colors;
    let text_color = colors.solid;
    let has_title =
        params.header.title.as_deref().is_some_and(|t| !t.trim().is_empty());
    let fields: Vec<_> = params
        .header
        .fields
        .iter()
        .filter(|f| !f.label.trim().is_empty())
        .collect();

    let mut y = params.margins_mm.top * super::model::MM_TO_PT;
    if has_title {
        let title = params.header.title.as_deref().unwrap_or_default().trim();
        let w = text_run_width(shaper, title, TITLE_SIZE);
        let x = geo.left + (geo.content_w - w) / 2.0;
        ops.extend(text_run_ops(shaper, title, x, y + TITLE_SIZE * 0.8, TITLE_SIZE, text_color, &params.font_id));
        y += 30.0;
    }
    if !fields.is_empty() {
        let slot = geo.content_w / fields.len() as f64;
        for (i, f) in fields.iter().enumerate() {
            let x = geo.left + i as f64 * slot;
            let baseline = y + FIELD_SIZE * 0.9;
            let mut pen = x;
            let label = format!("{}:", f.label.trim());
            ops.extend(text_run_ops(shaper, &label, pen, baseline, FIELD_SIZE, text_color, &params.font_id));
            pen += text_run_width(shaper, &label, FIELD_SIZE) + 3.0;
            let value = f.value.trim();
            if value.is_empty() {
                // 空值画下划线供手填
                ops.push(IrOp::Stroke {
                    path: IrPath::hline(pen, pen + BLANK_LINE_LEN, baseline + 1.5),
                    style: StrokeSpec { color: text_color, width: 0.6, dash: None },
                });
            } else {
                ops.extend(text_run_ops(shaper, value, pen, baseline, FIELD_SIZE, text_color, &params.font_id));
            }
        }
    }
}

/// 页码:底部居中,"第 i / N 页"(单页时不显示数字,留白)。
pub fn paint_page_number(
    ops: &mut Vec<IrOp>,
    params: &GenerateParams,
    shaper: &mut Shaper<'_>,
    index: usize,
    total: usize,
) {
    if total <= 1 {
        return;
    }
    let text = format!("第 {index} / {total} 页");
    let w = text_run_width(shaper, &text, PAGE_NO_SIZE);
    let x = (super::model::A4_W_PT - w) / 2.0;
    let y = super::model::A4_H_PT - params.margins_mm.bottom * super::model::MM_TO_PT * 0.45;
    ops.extend(text_run_ops(shaper, &text, x, y, PAGE_NO_SIZE, params.colors.solid, &params.font_id));
}
