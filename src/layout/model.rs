//! 领域模型:纸型、模式、生成参数与偏好。
//! 偏好与生成参数共用 `GenerateParams`(serde 全字段带默认值,生成时再做严格校验)。

use std::fmt;

use serde::{Deserialize, Serialize};

pub const A4_W_PT: f64 = 595.276;
pub const A4_H_PT: f64 = 841.89;
pub const MM_TO_PT: f64 = 72.0 / 25.4;

// ---------- 参数上限(生成时校验) ----------
pub const MAX_CLUSTERS: usize = 2000;
pub const MAX_REPEAT: u8 = 10;
pub const MAX_PAGES: usize = 60;
pub const MAX_TITLE_CHARS: usize = 40;
pub const MAX_FIELDS: usize = 3;
pub const MAX_FIELD_LABEL: usize = 20;
pub const MAX_FIELD_VALUE: usize = 30;

/// 纸型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PaperKind {
    /// 米字格
    #[serde(rename = "mizi")]
    #[default]
    MiZi,
    /// 田字格
    #[serde(rename = "tianzi")]
    TianZi,
    /// 回宫格
    #[serde(rename = "huigong")]
    HuiGong,
    /// 作文纸(连续填格,无逐字重复)
    #[serde(rename = "zuowen")]
    ZuoWen,
    /// 四线三格
    #[serde(rename = "sixian")]
    SiXian,
    /// 单横线
    #[serde(rename = "hengxian")]
    HengXian,
}

impl PaperKind {
    pub fn is_ruled(self) -> bool {
        matches!(self, PaperKind::SiXian | PaperKind::HengXian)
    }

    pub fn is_continuous(self) -> bool {
        self == PaperKind::ZuoWen
    }

    pub fn label(self) -> &'static str {
        match self {
            PaperKind::MiZi => "米字格",
            PaperKind::TianZi => "田字格",
            PaperKind::HuiGong => "回宫格",
            PaperKind::ZuoWen => "作文纸",
            PaperKind::SiXian => "四线三格",
            PaperKind::HengXian => "单横线",
        }
    }
}

/// 生成模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    /// 描红:浅色字供描写
    #[serde(rename = "miaohong")]
    #[default]
    MiaoHong,
    /// 抄写:空格供抄写(可选行首示范字)
    #[serde(rename = "chaoxie")]
    ChaoXie,
    /// 描红+抄写:前几格描红,其余空
    #[serde(rename = "hunhe")]
    HunHe,
    /// 纯字帖:每字实色一份
    #[serde(rename = "chuntie")]
    ChunTie,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::MiaoHong => "描红",
            Mode::ChaoXie => "抄写",
            Mode::HunHe => "描红+抄写",
            Mode::ChunTie => "纯字帖",
        }
    }
}

/// 文字方向:决定行内格子的排列顺序。Auto 按文本首个强方向字符判断;
/// TtbRtl 为传统竖排:列内从上到下,列序从右到左(仅方块格纸型生效)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Direction {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "ltr")]
    Ltr,
    #[serde(rename = "rtl")]
    Rtl,
    #[serde(rename = "ttb-rtl")]
    TtbRtl,
}

impl Direction {
    /// 是否按从右向左排(RTL 行或竖排列序)。TtbRtl 的判定与文本无关。
    pub fn is_rtl(self, text: &str) -> bool {
        match self {
            Direction::Rtl | Direction::TtbRtl => true,
            Direction::Ltr => false,
            Direction::Auto => first_strong_is_rtl(text),
        }
    }

    /// 是否竖排(仅方块格纸型;线格纸型忽略并退化为 LTR)。
    pub fn is_ttb(self, ruled: bool) -> bool {
        self == Direction::TtbRtl && !ruled
    }
}

/// 首个强方向字符是否属于 RTL 书写系统(希伯来/阿拉伯系)。
fn first_strong_is_rtl(text: &str) -> bool {
    for c in text.chars() {
        let cp = c as u32;
        let rtl = matches!(cp,
            0x0590..=0x05FF      // 希伯来
            | 0x0600..=0x06FF     // 阿拉伯
            | 0x0700..=0x074F     // 叙利亚
            | 0x0750..=0x077F     // 阿拉伯补充
            | 0x0860..=0x08FF     // 阿拉曼
            | 0xFB1D..=0xFDFF     // 希伯来呈现形式
            | 0xFE70..=0xFEFF     // 阿拉伯呈现形式
        );
        if rtl {
            return true;
        }
        // 常见 LTR 强方向字符直接定案(拉丁/希腊/西里尔/CJK 视作 LTR)
        if c.is_alphabetic() || (0x4E00..=0x9FFF).contains(&cp) {
            return false;
        }
    }
    false
}

/// RGB 颜色,序列化为 `#rrggbb`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color(#[serde(with = "color_hex")] pub [u8; 3]);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color([r, g, b])
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::rgb(0, 0, 0)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }
}

mod color_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(c: &[u8; 3], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 3], D::Error> {
        let s = String::deserialize(d)?;
        let s = s.trim_start_matches('#');
        if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom("颜色格式应为 #rrggbb"));
        }
        let v = u32::from_str_radix(s, 16).map_err(serde::de::Error::custom)?;
        Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorConfig {
    /// 格线
    #[serde(default = "ColorConfig::d_grid")]
    pub grid: Color,
    /// 实色字(纯字帖)
    #[serde(default = "ColorConfig::d_solid")]
    pub solid: Color,
    /// 描红字
    #[serde(default = "ColorConfig::d_light")]
    pub light: Color,
    /// 行首示范字
    #[serde(default = "ColorConfig::d_demo")]
    pub demo: Color,
}

impl ColorConfig {
    fn d_grid() -> Color {
        Color::rgb(0x9e, 0x9e, 0x9e)
    }
    fn d_solid() -> Color {
        Color::rgb(0x2b, 0x2b, 0x2b)
    }
    fn d_light() -> Color {
        // 传统描红用浅红色,浅灰在缩略/打印时对比度不足
        Color::rgb(0xd6, 0x8c, 0x8c)
    }
    fn d_demo() -> Color {
        Color::rgb(0x57, 0x57, 0x57)
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            grid: Self::d_grid(),
            solid: Self::d_solid(),
            light: Self::d_light(),
            demo: Self::d_demo(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Margins {
    #[serde(default = "Margins::d")]
    pub top: f64,
    #[serde(default = "Margins::d")]
    pub right: f64,
    #[serde(default = "Margins::d")]
    pub bottom: f64,
    #[serde(default = "Margins::d")]
    pub left: f64,
}

impl Margins {
    fn d() -> f64 {
        15.0
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 15.0,
            right: 15.0,
            bottom: 15.0,
            left: 15.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderField {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderConfig {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub fields: Vec<HeaderField>,
    #[serde(default = "HeaderConfig::d_true")]
    pub page_number: bool,
}

impl HeaderConfig {
    fn d_true() -> bool {
        true
    }
    pub fn is_empty(&self) -> bool {
        self.title.as_deref().is_none_or(|t| t.trim().is_empty())
            && self.fields.iter().all(|f| f.label.trim().is_empty())
            && !self.page_number
    }
}

impl Default for HeaderConfig {
    fn default() -> Self {
        Self {
            title: None,
            fields: Vec::new(),
            page_number: true,
        }
    }
}

/// 生成参数(= 持久化的用户偏好)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateParams {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub paper: PaperKind,
    #[serde(default)]
    pub mode: Mode,
    /// 每字描红格数(描红/混合模式),1..=10
    #[serde(default = "GenerateParams::d_trace")]
    pub trace_count: u8,
    /// 每字抄写格数(抄写/混合模式),1..=10
    #[serde(default = "GenerateParams::d_copy")]
    pub copy_count: u8,
    /// 抄写模式行首示范字
    #[serde(default = "GenerateParams::d_true")]
    pub row_leading_demo: bool,
    /// 汉字上方标注拼音(仅方块格纸型生效)
    #[serde(default)]
    pub pinyin: bool,
    /// 行内格子排列方向
    #[serde(default)]
    pub direction: Direction,
    #[serde(default = "GenerateParams::d_font")]
    pub font_id: String,
    /// 方块格边长 pt;None = 按纸型默认
    #[serde(default)]
    pub cell_size_pt: Option<f64>,
    /// 四线三格:单格高度 pt / 单横线:行距 pt;None = 默认
    #[serde(default)]
    pub line_height_pt: Option<f64>,
    /// 线格类纸型的格宽 pt;None = 默认
    #[serde(default)]
    pub cell_width_pt: Option<f64>,
    /// 每行格数;None = 按内容宽自动
    #[serde(default)]
    pub cols_per_row: Option<u16>,
    #[serde(default = "GenerateParams::d_col_gap")]
    pub col_gap_pt: f64,
    #[serde(default = "GenerateParams::d_row_gap")]
    pub row_gap_pt: f64,
    #[serde(default)]
    pub margins_mm: Margins,
    #[serde(default)]
    pub header: HeaderConfig,
    #[serde(default)]
    pub colors: ColorConfig,
}

impl GenerateParams {
    fn d_trace() -> u8 {
        3
    }
    fn d_copy() -> u8 {
        3
    }
    fn d_true() -> bool {
        true
    }
    fn d_font() -> String {
        "lxgw-wenkai".into()
    }
    fn d_col_gap() -> f64 {
        4.0
    }
    fn d_row_gap() -> f64 {
        6.0
    }

    /// 方块格边长(pt)。
    pub fn cell_size(&self) -> f64 {
        self.cell_size_pt.unwrap_or(15.0 * MM_TO_PT)
    }

    /// 线格类纸型:四线三格单格高 / 单横线行距(pt)。单横线需要更大行距承载整行手写体。
    pub fn line_height(&self) -> f64 {
        match self.paper {
            PaperKind::HengXian => self.line_height_pt.unwrap_or(10.0 * MM_TO_PT),
            _ => self.line_height_pt.unwrap_or(8.0 * MM_TO_PT),
        }
    }

    /// 线格类纸型的格宽(pt)。
    pub fn cell_width(&self) -> f64 {
        self.cell_width_pt.unwrap_or(12.0 * MM_TO_PT)
    }
}

/// 与 serde 默认值保持一致(derive Default 会给出 0/空,与反序列化语义不符)。
impl Default for GenerateParams {
    fn default() -> Self {
        Self {
            text: String::new(),
            paper: PaperKind::default(),
            mode: Mode::default(),
            trace_count: 3,
            copy_count: 3,
            row_leading_demo: true,
            pinyin: false,
            direction: Direction::default(),
            font_id: "lxgw-wenkai".into(),
            cell_size_pt: None,
            line_height_pt: None,
            cell_width_pt: None,
            cols_per_row: None,
            col_gap_pt: 4.0,
            row_gap_pt: 6.0,
            margins_mm: Margins::default(),
            header: HeaderConfig::default(),
            colors: ColorConfig::default(),
        }
    }
}
