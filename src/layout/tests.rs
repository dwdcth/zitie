//! 排版引擎单元测试:用真实内置字体(ShortStack ~100KB / LXGW WenKai)验证
//! 分页黄金值、模式计数、行首示范、作文纸连续填格与字号推导。

use super::ir::IrOp;
use super::model::*;
use super::paginate::{CellContent, Geometry};
use super::{layout_document, paginate, Shaper};
use crate::fonts::build_entry;

fn font_entry(name: &str) -> crate::fonts::FontEntry {
    let path = format!("{}/assets/fonts/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读取 {path} 失败(先运行 scripts/fetch-fonts.sh):{e}"));
    build_entry("test-font".into(), None, "Test".into(), "Regular".into(), "0".into(), path.into(), bytes)
        .expect("内置字体应能构建")
}

/// 固定几何的参数:默认 15mm 边距、gap 4/6;无页眉页码 → 各页行数一致
fn base_params() -> GenerateParams {
    GenerateParams {
        header: HeaderConfig { title: None, fields: vec![], page_number: false },
        ..Default::default()
    }
}

/// 上述参数下:content_w=510.24 → cols=11;content_h=756.85 → rows=15
#[test]
fn geometry_golden_values() {
    let p = base_params();
    let geo = Geometry::new(&p);
    assert_eq!(geo.cols, 11, "15mm 方格 4pt 间隙时每行 11 格");
    assert_eq!(geo.rows_first, 15);
    assert_eq!(geo.rows_rest, 15);
}

/// 描红模式:每字 trace_count 格,全部浅色
#[test]
fn miaohong_cells_and_pages() {
    let mut p = base_params();
    p.mode = Mode::MiaoHong;
    p.trace_count = 3;
    p.text = "永和九年".into(); // 4 字 × 3 = 12 格 → 1 行多一点 → 2 行
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].len(), 2, "12 格 / 11 列 = 2 行");
    assert_eq!(pages[0][0].len(), 11, "首行占满 11 格");
    assert_eq!(pages[0][1].len(), 1);
    assert!(pages[0][0].iter().all(|c| matches!(&c.content, CellContent::Light(_))));
}

/// 混合模式:每字 [描红×t, 空×c];行首无示范
#[test]
fn hunhe_pattern() {
    let mut p = base_params();
    p.mode = Mode::HunHe;
    p.trace_count = 2;
    p.copy_count = 2;
    p.text = "永".into();
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    let cells: Vec<&CellContent> = pages[0][0].iter().map(|c| &c.content).collect();
    assert_eq!(cells.len(), 4);
    assert!(matches!(cells[0], CellContent::Light(_)));
    assert!(matches!(cells[1], CellContent::Light(_)));
    assert_eq!(cells[2], &CellContent::Empty);
    assert_eq!(cells[3], &CellContent::Empty);
}

/// 抄写模式 + 行首示范:每行第 0 格为 Demo,其余为 Empty,配额不受示范影响
#[test]
fn chaoxie_row_leading_demo() {
    let mut p = base_params();
    p.mode = Mode::ChaoXie;
    p.copy_count = 4;
    p.row_leading_demo = true;
    p.text = "永和".into(); // 每字 4 空格 + 行首 1 示范
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    // 永和 = 2×4 = 8 空格 + 1 示范 = 9 格 → 1 行
    let row = &pages[0][0];
    assert_eq!(row.len(), 9);
    assert!(matches!(&row[0].content, CellContent::Demo(c) if c == "永"));
    assert!(row[1..].iter().all(|c| matches!(&c.content, CellContent::Empty)));

    // 大文本:跨行时每行首都是当前练习字的示范
    let mut p2 = p.clone();
    p2.text = "永".repeat(3); // 单字 12 空格 → 11 列放不下 → 2 行
    let pages2 = paginate::paginate(&p2, &geo);
    assert_eq!(pages2[0].len(), 2);
    assert!(matches!(&pages2[0][1][0].content, CellContent::Demo(c) if c == "永"));
}

/// 纯字帖:每字一实色格
#[test]
fn chuntie_one_solid_per_char() {
    let mut p = base_params();
    p.mode = Mode::ChunTie;
    p.text = "永和".into();
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    let cells: Vec<&CellContent> = pages[0][0].iter().map(|c| &c.content).collect();
    assert_eq!(cells.len(), 2);
    assert!(matches!(cells[0], CellContent::Solid(c) if c == "永"));
    assert!(matches!(cells[1], CellContent::Solid(c) if c == "和"));
}

/// 作文纸:连续填格,顺序保持,空格占空格,换行换行
#[test]
fn zuowen_continuous() {
    let mut p = base_params();
    p.paper = PaperKind::ZuoWen;
    p.cols_per_row = Some(6);
    p.text = "永和九年\n岁在".into();
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    assert_eq!(pages.len(), 1);
    let row0: Vec<String> = pages[0][0]
        .iter()
        .map(|c| match &c.content {
            CellContent::Light(s) | CellContent::Solid(s) | CellContent::Demo(s) => s.clone(),
            CellContent::Empty => String::new(),
        })
        .collect();
    assert_eq!(row0, vec!["永", "和", "九", "年"], "连续模式换行即止,不补空格");
    // 换行 → 第二行从新行开始
    assert!(matches!(&pages[0][1][0].content, CellContent::Light(s) if s == "岁"));
}

/// 空文本 → 一页空白(0 行),渲染层画满空白格子
#[test]
fn empty_text_blank_page() {
    let p = base_params();
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].is_empty());
}

/// 分页跨页黄金值:11 列 × 15 行/页,描红×5 → 50 字 = 250 格 → 23 行 → 2 页
#[test]
fn pagination_golden_pages() {
    let mut p = base_params();
    p.mode = Mode::HunHe;
    p.trace_count = 3;
    p.copy_count = 2; // 每字 5 格
    p.text = "永".repeat(50);
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    assert_eq!(pages.len(), 2, "250 格 → 23 行 → 首页 15 行 + 次页 8 行");
    assert_eq!(pages[0].len(), 15);
    assert_eq!(pages[1].len(), 8);
}

/// 页数上限:应报 TooManyPages
#[test]
fn too_many_pages_rejected() {
    let mut p = base_params();
    p.mode = Mode::HunHe;
    p.trace_count = 10;
    p.copy_count = 10;
    p.text = "永".repeat(1000);
    let entry = font_entry("ShortStack-Regular.ttf");
    let err = layout_document(&p, &entry).unwrap_err();
    assert!(matches!(err, super::LayoutError::TooManyPages), "got: {err:?}");
}

/// 页码回填与页眉只在首页
#[test]
fn header_first_page_and_page_numbers() {
    let mut p = base_params();
    p.mode = Mode::HunHe;
    p.trace_count = 3;
    p.copy_count = 2;
    p.text = "永".repeat(50);
    p.header = HeaderConfig {
        title: Some("兰亭集序".into()),
        fields: vec![HeaderField { label: "班级".into(), value: String::new() }],
        page_number: true,
    };
    let entry = font_entry("LXGWWenKai-Regular.ttf"); // 标题是中文,用中文字体
    let doc = layout_document(&p, &entry).unwrap();
    assert_eq!(doc.page_count(), 2);
    assert_eq!(doc.pages[0].index, 1);
    assert_eq!(doc.pages[1].total, 2);

    // 首页有 18pt 的标题字(格子内字符约 30pt,不能只比最大值),第二页没有
    let has_title_size = |page: &crate::layout::ir::PageIr| {
        page.ops.iter().any(|op| match op {
            IrOp::Glyphs { size, .. } => (*size - 18.0).abs() < 0.01,
            _ => false,
        })
    };
    assert!(has_title_size(&doc.pages[0]), "首页有 18pt 标题");
    assert!(!has_title_size(&doc.pages[1]), "第二页无标题");
}

/// 米字格装饰线:外框(实线)+ 十字 + 对角(虚线)
#[test]
fn mizi_frame_has_solid_rect_and_dashed_guides() {
    let mut p = base_params();
    p.text = "永".into();
    p.cols_per_row = Some(4);
    let entry = font_entry("ShortStack-Regular.ttf");
    let doc = layout_document(&p, &entry).unwrap();
    let strokes: Vec<&super::ir::StrokeSpec> = doc.pages[0]
        .ops
        .iter()
        .filter_map(|op| match op {
            IrOp::Stroke { style, .. } => Some(style),
            _ => None,
        })
        .collect();
    assert!(strokes.iter().any(|s| s.dash.is_none()), "有实线外框");
    assert!(strokes.iter().any(|s| s.dash.is_some()), "有虚线辅助线");
}

/// 四线三格:每行恰好 4 条横线,字母基线落在第 3 条线上
#[test]
fn sixian_four_lines_and_baseline() {
    let mut p = base_params();
    p.paper = PaperKind::SiXian;
    p.text = "abc".into();
    p.cols_per_row = Some(16); // 9 格单行放得下,全部字母同一基线
    let entry = font_entry("ShortStack-Regular.ttf");
    let doc = layout_document(&p, &entry).unwrap();

    // 第一行的 4 条横线(排序后前 4 个不重复 y)
    let mut line_ys: Vec<f64> = Vec::new();
    for op in &doc.pages[0].ops {
        if let IrOp::Stroke { path, .. } = op {
            if let [super::ir::PathEl::Move(x0, y), super::ir::PathEl::Line(x1, y1)] = &path.els[..] {
                if (y - y1).abs() < 1e-9 && x1 > x0 && !line_ys.iter().any(|v| (*v - y).abs() < 0.01) {
                    line_ys.push(*y);
                }
            }
        }
    }
    line_ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(line_ys.len() >= 4, "至少 4 条线,got {:?}", line_ys.len());
    let h = p.line_height();
    for w in line_ys[..4].windows(2) {
        assert!((w[1] - w[0] - h).abs() < 0.01, "首行线距 = 行高 {h}");
    }

    // 字母 origin 的 y 应等于第 3 条线(baseline)
    let baseline = line_ys[2];
    for op in &doc.pages[0].ops {
        if let IrOp::Glyphs { origin, .. } = op {
            assert!((origin.1 - baseline).abs() < 0.01, "字母基线对齐第 3 条线");
        }
    }
}

/// 四线三格字号:x 高度填满中格,或被 ascender/descender 钳制(都不越线)
#[test]
fn sixian_font_size_fits_x_zone() {
    let mut p = base_params();
    p.paper = PaperKind::SiXian;
    p.text = "x".into();
    let entry = font_entry("ShortStack-Regular.ttf");
    let doc = layout_document(&p, &entry).unwrap();
    let size = doc
        .pages[0]
        .ops
        .iter()
        .find_map(|op| match op {
            IrOp::Glyphs { size, .. } => Some(*size),
            _ => None,
        })
        .expect("应有字母");
    let shaper = Shaper::new(&entry);
    let info = shaper.face_info();
    let upm = info.units_per_em;
    let x_em = info.x_height.unwrap() / upm;
    let asc_em = info.ascender / upm;
    let h = p.line_height();
    assert!(size * x_em <= h + 0.5, "x 高不越中格:{}", size * x_em);
    assert!(size * asc_em <= 2.0 * h + 0.5, "ascender 不越顶线:{}", size * asc_em);
    // 未被钳制时应恰好填满中格
    if size * asc_em < 2.0 * h - 0.5 {
        assert!((size * x_em - h).abs() < 0.5);
    }
}

/// 调试:整形器基础行为(供排障,保留作回归)
#[test]
fn shaper_basic() {
    let entry = font_entry("ShortStack-Regular.ttf");
    let mut shaper = Shaper::new(&entry);
    let shaped = shaper.shape("x");
    assert!(!shaped.glyphs.is_empty(), "x 应整形出 glyph: {:?}", shaped);
    assert!(shaped.ink.is_some(), "x 应有墨迹框");
    let info = shaper.face_info();
    assert!(info.units_per_em > 0.0);
    assert!(info.x_height.is_some());
}

/// 端到端 IR 冒烟:中文 + LXGW 文楷,多纸型非空
#[test]
fn layout_smoke_all_papers() {
    let entry = font_entry("LXGWWenKai-Regular.ttf");
    for paper in [PaperKind::MiZi, PaperKind::TianZi, PaperKind::HuiGong, PaperKind::ZuoWen, PaperKind::SiXian, PaperKind::HengXian] {
        let mut p = base_params();
        p.paper = paper;
        p.text = "永和九年岁在癸丑".into();
        let doc = layout_document(&p, &entry).unwrap_or_else(|e| panic!("{paper:?} 布局失败:{e}"));
        assert!(!doc.pages[0].ops.is_empty(), "{paper:?} 首页应有绘制指令");
        assert!(
            doc.pages[0].ops.iter().any(|op| matches!(op, IrOp::Glyphs { .. })),
            "{paper:?} 应有字形指令"
        );
    }
}



/// 练习纸中空格占一个空格(词间分隔/休整格)
#[test]
fn space_occupies_a_cell_in_practice_paper() {
    let mut p = base_params();
    p.mode = Mode::MiaoHong;
    p.trace_count = 1;
    p.text = "ab cd".into(); // a,b,空格,c,d → 5 格
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    assert_eq!(pages[0][0].len(), 5);
    assert_eq!(pages[0][0][2].content, CellContent::Empty, "第 3 格应为空格分隔");
    assert!(matches!(&pages[0][0][3].content, CellContent::Light(c) if c == "c"));
}
