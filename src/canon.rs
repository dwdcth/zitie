//! 参数规范化与内容寻址哈希:同参数(语义等价)必得同哈希,
//! PDF 因此可以按哈希永久缓存(immutable + ETag)。

use crate::layout::model::{GenerateParams, Margins};
use sha2::{Digest, Sha256};

/// 哈希域版本前缀:排版/渲染引擎行为变更时递增,整体失效旧缓存
/// (v2:修复 fill 状态泄漏;v3:线格纸格宽按字体推导、单横线重排、新增竖排;
///  v4:单横线改为下伸部底线,字母不再穿过横线)。
const HASH_DOMAIN: &[u8] = b"zitie-v4\0";

/// 规范化:CRLF→LF、去首尾空白、浮点取两位小数、清理空字段。
pub fn canonicalize(p: &GenerateParams) -> GenerateParams {
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    let mut header = p.header.clone();
    header.title = header
        .title
        .as_ref()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    header.fields.retain(|f| !f.label.trim().is_empty());
    for f in &mut header.fields {
        f.label = f.label.trim().to_string();
        f.value = f.value.trim().to_string();
    }
    GenerateParams {
        text: p.text.replace("\r\n", "\n").trim().to_string(),
        header,
        cell_size_pt: p.cell_size_pt.map(round2),
        line_height_pt: p.line_height_pt.map(round2),
        cell_width_pt: p.cell_width_pt.map(round2),
        col_gap_pt: round2(p.col_gap_pt),
        row_gap_pt: round2(p.row_gap_pt),
        margins_mm: Margins {
            top: round2(p.margins_mm.top),
            right: round2(p.margins_mm.right),
            bottom: round2(p.margins_mm.bottom),
            left: round2(p.margins_mm.left),
        },
        ..p.clone()
    }
}

/// 规范 JSON 序列化(用于落盘与哈希)。
pub fn canonical_json(p: &GenerateParams) -> String {
    serde_json::to_string(&canonicalize(p)).expect("GenerateParams 序列化不应失败")
}

/// sha256(域前缀 + 规范 JSON),hex 64 位。
pub fn hash_params(p: &GenerateParams) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN);
    hasher.update(canonical_json(p).as_bytes());
    hex::encode(hasher.finalize())
}

/// 校验哈希格式(64 位小写 hex),防路径穿越。
pub fn is_valid_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_params_same_hash() {
        let a = GenerateParams::default();
        let b = GenerateParams::default();
        assert_eq!(hash_params(&a), hash_params(&b));
    }

    #[test]
    fn canonicalization_collapses_equivalents() {
        let a = GenerateParams { text: "  永\r\n和\n".into(), ..Default::default() };
        let b = GenerateParams { text: "永\n和".into(), ..Default::default() };
        assert_eq!(hash_params(&a), hash_params(&b));
    }

    #[test]
    fn float_noise_collapsed() {
        let a = GenerateParams { col_gap_pt: 4.0, ..Default::default() };
        let b = GenerateParams { col_gap_pt: 3.9999999999, ..Default::default() };
        assert_eq!(hash_params(&a), hash_params(&b));
    }

    #[test]
    fn hash_format() {
        assert!(is_valid_hash(&hash_params(&GenerateParams::default())));
        assert!(!is_valid_hash("../etc/passwd"));
        assert!(!is_valid_hash("ABC"));
    }
}
