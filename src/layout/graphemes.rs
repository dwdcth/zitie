//! 文本 → 记号序列:grapheme cluster 切分与空白/换行策略。

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// 一个 grapheme cluster(emoji ZWJ 序列、组合音标等各算一个)
    Cluster(String),
    /// 空格/全角空格:作文纸占一个空格,练习纸跳过
    Space,
    /// 强制换行
    Newline,
}

pub fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        for g in line.graphemes(true) {
            let trimmed = g.trim_matches('\u{feff}');
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.chars().all(|c| c == ' ' || c == '\u{3000}') {
                // 连续空格合并为一个 Space 记号
                if out.last() != Some(&Token::Space) {
                    out.push(Token::Space);
                }
            } else {
                out.push(Token::Cluster(trimmed.to_string()));
            }
        }
        out.push(Token::Newline);
    }
    // 结尾的换行记号去掉
    while matches!(out.last(), Some(Token::Newline)) {
        out.pop();
    }
    out
}

/// 统计 cluster 数(用于上限校验与前端计数)。
pub fn cluster_count(text: &str) -> usize {
    tokenize(text).into_iter().filter(|t| matches!(t, Token::Cluster(_) | Token::Space)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_zwj_is_one_cluster() {
        let t = tokenize("👨‍👩‍👧‍👦");
        assert_eq!(t, vec![Token::Cluster("👨‍👩‍👧‍👦".into())]);
    }

    #[test]
    fn combining_mark_is_one_cluster() {
        assert_eq!(tokenize("e\u{301}"), vec![Token::Cluster("e\u{301}".into())]);
        assert_eq!(tokenize("か\u{3099}"), vec![Token::Cluster("か\u{3099}".into())]);
    }

    #[test]
    fn spaces_and_newlines() {
        let t = tokenize("永  和\n\n田");
        assert_eq!(
            t,
            vec![
                Token::Cluster("永".into()),
                Token::Space,
                Token::Cluster("和".into()),
                Token::Newline,
                Token::Newline,
                Token::Cluster("田".into()),
            ]
        );
    }

    #[test]
    fn trailing_newline_dropped() {
        assert_eq!(tokenize("永\n"), vec![Token::Cluster("永".into())]);
    }

    #[test]
    fn fullwidth_space_is_space() {
        assert_eq!(tokenize("a\u{3000}b"), vec![
            Token::Cluster("a".into()),
            Token::Space,
            Token::Cluster("b".into()),
        ]);
    }
}
