//! 字体度量与单 cluster 整形:用 skrifa 读度量/墨迹框,用 rustybuzz
//! 对单个 grapheme cluster 整形(GSUB/GPOS,支持印度系记号、阿拉伯 cluster 内连写)。

use skrifa::instance::{LocationRef, Size};
use skrifa::raw::tables::name::NameId;
use skrifa::{GlyphId, MetadataProvider};

/// 字体基础度量,全部为 font units。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FontFaceInfo {
    pub units_per_em: f64,
    /// 通常为正
    pub ascender: f64,
    /// 通常为负
    pub descender: f64,
    pub x_height: Option<f64>,
    pub cap_height: Option<f64>,
}

/// glyph 墨迹框(font units,y 向上)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkBox {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

impl InkBox {
    pub fn width(&self) -> f64 {
        self.x_max - self.x_min
    }
    pub fn height(&self) -> f64 {
        self.y_max - self.y_min
    }
    pub fn union(self, other: InkBox) -> InkBox {
        InkBox {
            x_min: self.x_min.min(other.x_min),
            y_min: self.y_min.min(other.y_min),
            x_max: self.x_max.max(other.x_max),
            y_max: self.y_max.max(other.y_max),
        }
    }
}

/// 整形后的单个 glyph:几何量已按 em 归一化(除以 units_per_em)。
#[derive(Debug, Clone)]
pub struct PlacedGlyph {
    pub glyph_id: u16,
    pub x_advance: f64,
    pub x_offset: f64,
    pub y_offset: f64,
    /// 该 glyph 的 cluster 在 cluster 字符串中的起始字节
    pub byte_start: usize,
}

/// 一个 grapheme cluster 的整形结果:glyph 序列 + 合并墨迹框(em 单位)。
#[derive(Debug, Clone)]
pub struct ShapedCluster {
    pub glyphs: Vec<PlacedGlyph>,
    /// 合并墨迹框(em 单位,含各 glyph 的 shaping offset),无轮廓时为 None
    pub ink: Option<InkBox>,
}

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("不是有效的字体文件(仅支持 .ttf/.otf/.ttc)")]
    NotAFont,
    #[error("字体文件损坏或无法解析:{0}")]
    Parse(String),
}

/// 解析字体(取第 index 个 face)。魔数校验:0x00010000(ttf)、OTTO(otf/cff)、ttcf(collection)。
pub fn parse_font(bytes: &[u8], index: u32) -> Result<skrifa::FontRef<'_>, FontError> {
    if bytes.len() < 12 {
        return Err(FontError::NotAFont);
    }
    let magic = &bytes[..4];
    let known = magic == [0x00, 0x01, 0x00, 0x00]
        || magic == b"OTTO"
        || magic == b"ttcf"
        || magic == b"true"
        || magic == b"typ1";
    if !known {
        return Err(FontError::NotAFont);
    }
    skrifa::FontRef::from_index(bytes, index).map_err(|e| FontError::Parse(e.to_string()))
}

/// 读取字体基础度量(font units)。
pub fn face_info(font: &skrifa::FontRef<'_>) -> Result<FontFaceInfo, FontError> {
    // Size::unscaled + 默认实例 = 字体设计单位下的度量
    let m = font.metrics(Size::unscaled(), LocationRef::default());
    let upm = m.units_per_em as f64;
    if upm <= 0.0 {
        return Err(FontError::Parse("units_per_em 无效".into()));
    }
    // x/cap 高度优先取 OS/2 度量,缺失时回退到 'x'/'H' 墨迹实测
    let x_height = m
        .x_height
        .map(|v| v as f64)
        .filter(|v| *v > 0.0)
        .or_else(|| ink_height(font, 'x'));
    let cap_height = m
        .cap_height
        .map(|v| v as f64)
        .filter(|v| *v > 0.0)
        .or_else(|| ink_height(font, 'H'));
    Ok(FontFaceInfo {
        units_per_em: upm,
        ascender: m.ascent as f64,
        descender: m.descent as f64,
        x_height,
        cap_height,
    })
}

fn ink_height(font: &skrifa::FontRef<'_>, c: char) -> Option<f64> {
    let gid = font.charmap().map(c)?;
    let b = ink_box(font, gid.to_u32() as u16)?;
    Some(b.height())
}

/// 收集轮廓路径点的包围盒画笔(font units,y 向上)。
/// 二次/三次曲线用控制点近似(对居中缩放用途足够)。
#[derive(Default)]
struct BoundsPen {
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
    any: bool,
}

impl BoundsPen {
    fn include(&mut self, x: f32, y: f32) {
        if !self.any {
            self.x_min = x;
            self.x_max = x;
            self.y_min = y;
            self.y_max = y;
            self.any = true;
            return;
        }
        self.x_min = self.x_min.min(x);
        self.x_max = self.x_max.max(x);
        self.y_min = self.y_min.min(y);
        self.y_max = self.y_max.max(y);
    }
}

impl skrifa::outline::OutlinePen for BoundsPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.include(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.include(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.include(cx0, cy0);
        self.include(x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.include(cx0, cy0);
        self.include(cx1, cy1);
        self.include(x, y);
    }
    fn close(&mut self) {}
}

/// 读取字形墨迹框(font units):遍历轮廓路径点取包围盒。
pub fn ink_box(font: &skrifa::FontRef<'_>, gid: u16) -> Option<InkBox> {
    let og = font.outline_glyphs().get(GlyphId::new(gid as u32))?;
    let mut pen = BoundsPen::default();
    og.draw(
        skrifa::outline::DrawSettings::from(Size::unscaled()),
        &mut pen,
    )
    .ok()?;
    if !pen.any {
        return None;
    }
    Some(InkBox {
        x_min: pen.x_min as f64,
        y_min: pen.y_min as f64,
        x_max: pen.x_max as f64,
        y_max: pen.y_max as f64,
    })
}

/// 读取字体家族名/子家族名。
pub fn read_name(font: &skrifa::FontRef<'_>, name: FontName) -> Option<String> {
    let id = match name {
        FontName::Family => NameId::FAMILY_NAME,
        FontName::Subfamily => NameId::SUBFAMILY_NAME,
    };
    let mut strings = font.localized_strings(id);
    strings
        .find(|s| matches!(s.language(), Some(lang) if lang.starts_with("en")))
        .or_else(|| strings.next())
        .map(|s| s.to_string())
}

pub enum FontName {
    Family,
    Subfamily,
}

/// 对单个 grapheme cluster 整形,返回 em 归一化的 glyph 序列与合并墨迹框(em 单位)。
/// `face` 由调用方按请求构建(生命周期绑定字体字节)。
pub fn shape_cluster(
    face: &rustybuzz::Face<'_>,
    font: &skrifa::FontRef<'_>,
    text: &str,
) -> ShapedCluster {
    let upm = face.units_per_em() as f64;
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(text);
    let shaped = rustybuzz::shape(face, &[], buf);

    let infos = shaped.glyph_infos();
    let positions = shaped.glyph_positions();
    let mut glyphs = Vec::with_capacity(infos.len());
    let mut ink: Option<InkBox> = None;

    for (info, pos) in infos.iter().zip(positions.iter()) {
        let gid = info.glyph_id as u16;
        let x_off = pos.x_offset as f64 / upm;
        let y_off = pos.y_offset as f64 / upm;
        glyphs.push(PlacedGlyph {
            glyph_id: gid,
            x_advance: pos.x_advance as f64 / upm,
            x_offset: x_off,
            y_offset: y_off,
            byte_start: (info.cluster as usize).min(text.len()),
        });
        // 合并墨迹框:glyph 框 + shaping 偏移(转 font units)
        if let Some(b) = ink_box(font, gid) {
            let shifted = InkBox {
                x_min: b.x_min + x_off * upm,
                y_min: b.y_min + y_off * upm,
                x_max: b.x_max + x_off * upm,
                y_max: b.y_max + y_off * upm,
            };
            ink = Some(match ink {
                Some(acc) => acc.union(shifted),
                None => shifted,
            });
        }
    }

    ShapedCluster {
        glyphs,
        ink: ink.map(|b| InkBox {
            x_min: b.x_min / upm,
            y_min: b.y_min / upm,
            x_max: b.x_max / upm,
            y_max: b.y_max / upm,
        }),
    }
}

/// 检查字符是否被字体覆盖(用于生成前的缺字校验)。
pub fn covers_char(font: &skrifa::FontRef<'_>, c: char) -> bool {
    matches!(font.charmap().map(c), Some(g) if g != GlyphId::NOTDEF)
}
