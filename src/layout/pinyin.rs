//! 拼音标注:单汉字 cluster → 带声调拼音;非汉字返回 None。
//! 词典来自 pinyin crate(内嵌,多音字取常用读音,不做上下文消歧)。

use pinyin::ToPinyin;

/// cluster 的拼音(带声调,如 "yǒng")。
/// 仅处理「恰好一个汉字」的 cluster;emoji/组合记号/拉丁字母等返回 None。
pub fn pinyin_of(cluster: &str) -> Option<String> {
    let mut chars = cluster.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None; // 多字符 cluster(组合音标等)不标拼音
    }
    let py = first.to_pinyin()?;
    Some(py.with_tone().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_hanzi_gets_toned_pinyin() {
        assert_eq!(pinyin_of("永"), Some("yǒng".into()));
        assert_eq!(pinyin_of("和"), Some("hé".into()));
    }

    #[test]
    fn non_hanzi_returns_none() {
        assert_eq!(pinyin_of("A"), None);
        assert_eq!(pinyin_of("。"), None);
        assert_eq!(pinyin_of("e\u{301}"), None); // 组合音标 cluster
        assert_eq!(pinyin_of("👨‍👩‍👧‍👦"), None);
        assert_eq!(pinyin_of("和了"), None); // 多字符 cluster
    }
}
