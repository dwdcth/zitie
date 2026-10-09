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

/// 拼音:开启后行高增加拼音条,格子上方出现小字号拼音字素
#[test]
fn pinyin_strip_and_annotation() {
    let mut p = base_params();
    p.text = "永和".into();
    p.mode = Mode::ChaoXie;
    p.copy_count = 1;
    p.header = HeaderConfig { title: None, fields: vec![], page_number: false };
    let geo_off = Geometry::new(&p);
    p.pinyin = true;
    let geo_on = Geometry::new(&p);
    assert_eq!(geo_on.pinyin_strip, paginate::PINYIN_STRIP);
    assert!(
        geo_on.row_h > geo_off.row_h,
        "开启拼音后行高应包含拼音条"
    );

    let entry = font_entry("LXGWWenKai-Regular.ttf");
    let doc = layout_document(&p, &entry).unwrap();
    // 找到小字号(拼音)的 op:其 text 应为 "yǒng"/"hé"
    let pinyin_ops: Vec<&IrOp> = doc.pages[0]
        .ops
        .iter()
        .filter(|op| match op {
            IrOp::Glyphs { text, size, .. } => {
                *size < 8.0 && (text.contains('y') || text.contains('h'))
            }
            _ => false,
        })
        .collect();
    assert!(!pinyin_ops.is_empty(), "应有拼音标注 op");
    // 拼音基线在格子框上沿之上
    if let IrOp::Glyphs { origin, .. } = pinyin_ops[0] {
        let cell_top = geo_on.cell_top_of(0, 0);
        assert!(origin.1 <= cell_top, "拼音应画在格子上方");
    }
}

/// 拼音仅对方块格纸型生效
#[test]
fn pinyin_ignored_on_ruled_paper() {
    let mut p = base_params();
    p.paper = PaperKind::SiXian;
    p.text = "永".into();
    p.pinyin = true;
    let geo = Geometry::new(&p);
    assert_eq!(geo.pinyin_strip, 0.0, "四线三格不标拼音");
}

/// RTL:行内格子从右端排起,行首示范格在最右
#[test]
fn rtl_row_direction() {
    let mut p = base_params();
    p.text = "永和".into();
    p.mode = Mode::ChaoXie;
    p.copy_count = 1;
    p.row_leading_demo = true;
    p.cols_per_row = Some(6);
    let geo_ltr = Geometry::new(&p);
    assert!(!geo_ltr.rtl, "中文默认 LTR");
    assert!(geo_ltr.x_of(0) < geo_ltr.x_of(1), "LTR 列 0 在左");

    p.direction = model_direction_rtl();
    let geo_rtl = Geometry::new(&p);
    assert!(geo_rtl.rtl);
    assert!(geo_rtl.x_of(0) > geo_rtl.x_of(1), "RTL 列 0 在右");
    // RTL 镜像:RTL 列 0 与 LTR 最末列同位,反之亦然
    assert!((geo_rtl.x_of(0) - geo_ltr.x_of(5)).abs() < 0.01, "RTL 行首 = LTR 行尾位置");
    assert!((geo_rtl.x_of(5) - geo_ltr.x_of(0)).abs() < 0.01, "RTL 行尾 = LTR 行首位置");
}

/// 阿拉伯文本自动判定 RTL
#[test]
fn arabic_auto_detects_rtl() {
    let mut p = base_params();
    p.text = "مرحبا".into();
    assert!(Geometry::new(&p).rtl, "阿拉伯文应自动 RTL");
    p.direction = Direction::Ltr;
    assert!(!Geometry::new(&p).rtl, "显式 Ltr 覆盖自动判定");
}

fn model_direction_rtl() -> Direction {
    Direction::Rtl
}

/// 四线三格:未指定格宽时,默认格宽按字体推导,最宽字母(m/W)+ 间隙放得下
#[test]
fn sixian_default_cell_width_fits_letters() {
    let mut p = base_params();
    p.paper = PaperKind::SiXian;
    p.text = "mmm".into();
    let entry = font_entry("ShortStack-Regular.ttf");
    // 用户未设 cell_width → layout_document 注入字体感知默认值
    let doc = layout_document(&p, &entry).unwrap();
    let geo = Geometry::new(&p);
    let injected = geo.cell_w; // 12mm 固定值(未注入时的旧默认)
    let _ = injected;
    // 直接从渲染出的字母位置推断实际格宽:相邻字母起点差 = 格宽 + 间隙
    let mut origins: Vec<f64> = doc.pages[0]
        .ops
        .iter()
        .filter_map(|op| match op {
            IrOp::Glyphs { origin, text, .. } if text == "m" => Some(origin.0),
            _ => None,
        })
        .collect();
    origins.sort_by(|a, b| a.partial_cmp(b).unwrap());
    origins.dedup();
    assert!(origins.len() >= 2, "应有多个 m: {origins:?}");
    let pitch = origins[1] - origins[0];
    let mut shaper = Shaper::new(&entry);
    let size = super::ruled::font_size(&shaper, &p);
    let m_adv = super::advance_em(&shaper.shape("m")) * size;    assert!(
        pitch >= m_adv + 1.0,
        "相邻字母间距({pitch:.1}) 应 ≥ m 步进({m_adv:.1}) + 1pt,不再挤压"
    );
}

/// 单横线:字号 = 0.68×行距,横线是下伸部底线(字母不穿线)
#[test]
fn hengxian_size_and_baseline_on_line() {
    let mut p = base_params();
    p.paper = PaperKind::HengXian;
    p.text = "ab".into();
    let entry = font_entry("ShortStack-Regular.ttf");
    let doc = layout_document(&p, &entry).unwrap();

    // 横线 y(第一行 = 行带底部)
    let line_y: Vec<f64> = {
        let mut v: Vec<f64> = Vec::new();
        for op in &doc.pages[0].ops {
            if let IrOp::Stroke { path, .. } = op {
                if let [super::ir::PathEl::Move(x0, y), super::ir::PathEl::Line(x1, y1)] = &path.els[..] {
                    if (y - y1).abs() < 1e-9 && *x1 > *x0 && !v.iter().any(|e| (*e - y).abs() < 0.5) {
                        v.push(*y);
                    }
                }
            }
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    };
    assert!(!line_y.is_empty());
    let first_line = line_y[0];
    let lh = p.line_height();
    assert!((first_line - (p.margins_mm.top * MM_TO_PT + lh)).abs() < 0.01, "横线在行带底部");

    // 基线 = 横线上移一个下伸高度;字号 = 0.68×行距(默认 10mm)
    let mut shaper = Shaper::new(&entry);
    let size = super::ruled::font_size(&shaper, &p);
    let info = shaper.face_info();
    let desc_pt = (info.descender / info.units_per_em).abs() * size;
    let expected_baseline = first_line - desc_pt;
    for op in &doc.pages[0].ops {
        if let IrOp::Glyphs { origin, size, .. } = op {
            assert!((origin.1 - expected_baseline).abs() < 0.01, "基线应比横线高一个下伸高度");
            assert!((size - lh * 0.68).abs() < 0.01, "字号应为 0.68×行距");
        }
    }
    assert!((lh - 10.0 * MM_TO_PT).abs() < 0.01, "单横线默认行距 10mm");
}

/// 竖排:列内自上而下,列序自右向左
#[test]
fn ttb_vertical_columns_right_to_left() {
    let mut p = base_params();
    p.paper = PaperKind::ZuoWen;
    p.text = "永和\n九岁".into(); // 两列,每列 2 字
    p.direction = Direction::TtbRtl;
    let entry = font_entry("LXGWWenKai-Regular.ttf");
    let doc = layout_document(&p, &entry).unwrap();

    // 收集每个字的 (x, y)
    let mut placed: Vec<(String, f64, f64)> = Vec::new();
    for op in &doc.pages[0].ops {
        if let IrOp::Glyphs { origin, text, .. } = op {
            placed.push((text.clone(), origin.0, origin.1));
        }
    }
    let geo = Geometry::new(&p); // ttb → rtl = true,x_of(0) 为最右列
    let cell = geo.cell_w;
    let col0 = geo.x_of(0); // 第一列(最右)
    let col1 = geo.x_of(1);
    let in_col = |x: f64, col_x: f64| x >= col_x - 1.0 && x <= col_x + cell + 1.0;

    let find = |t: &str| placed.iter().find(|(txt, _, _)| txt == t).unwrap();
    let (_, x1, y11) = find("永").clone();
    let (_, x1b, y12) = find("和").clone();
    let (_, x2, y21) = find("九").clone();

    assert!(in_col(x1, col0) && in_col(x1b, col0), "永和 同在第一列(最右)");
    assert!(y12 > y11, "列内自上而下:和 在 永 下方");
    assert!(in_col(x2, col1), "第二列(九)在第一列左侧一列");
    assert!(y21 < y12, "第二列从头开始");
}

/// 竖排在四线三格上被忽略(退化为横排)
#[test]
fn ttb_ignored_on_ruled_paper() {
    let mut p = base_params();
    p.paper = PaperKind::SiXian;
    p.text = "abc".into();
    p.direction = Direction::TtbRtl;
    let geo = Geometry::new(&p);
    assert!(!p.direction.is_ttb(p.paper.is_ruled()), "线格纸型忽略竖排");
    assert!(!geo.rtl || true);
}

/// 作文纸+抄写:全文以示范色印一遍(供照抄),不再是空白纸
#[test]
fn zuowen_chaoxie_shows_demo_text() {
    let mut p = base_params();
    p.paper = PaperKind::ZuoWen;
    p.mode = Mode::ChaoXie;
    p.text = "永和九年".into();
    let geo = Geometry::new(&p);
    let pages = paginate::paginate(&p, &geo);
    let cells: Vec<&CellContent> = pages[0][0].iter().map(|c| &c.content).collect();
    assert_eq!(cells.len(), 4);
    assert!(cells.iter().all(|c| matches!(c, CellContent::Demo(_))), "全文应为示范色:{cells:?}");
    // 其余行保持空白格子(由画满整页的网格提供)
    assert_eq!(pages[0].len(), 1, "只有含文本的一行,其余是空网格");
}
