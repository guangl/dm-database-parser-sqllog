use daachorse::DoubleArrayAhoCorasick;

/// 围绕 daachorse::DoubleArrayAhoCorasick 的简单适配器。
/// 存储原始模式（按顺序），并提供一个辅助方法
/// 用于获取每个模式在输入中首次出现的位置。
pub struct Matcher {
    ac: DoubleArrayAhoCorasick<usize>,
    patterns: Vec<String>,
}

impl Matcher {
    /// 从一组模式构建一个 Matcher（模式顺序重要）。
    /// 空模式会被忽略；至少需要一个非空模式。
    pub fn from_patterns<S: AsRef<str>>(patterns: &[S]) -> Self {
        // 收集所有权的非空模式
        let patterns_owned: Vec<String> = patterns
            .iter()
            .map(|s| s.as_ref().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if patterns_owned.is_empty() {
            panic!("failed to build daachorse automaton: no non-empty patterns provided");
        }

        // 构建有所有权的字节缓冲，以便向 daachorse 传递切片
        let pats_bufs: Vec<Vec<u8>> = patterns_owned.iter().map(|s| s.as_bytes().to_vec()).collect();
        let pats_slices: Vec<&[u8]> = pats_bufs.iter().map(|v| v.as_slice()).collect();

        let ac = DoubleArrayAhoCorasick::new(&pats_slices)
            .unwrap_or_else(|e| panic!("failed to build daachorse automaton: {}", e));

        Matcher { ac, patterns: patterns_owned }
    }

    /// 返回一个 Vec<Option<usize>>，表示每个模式的首次匹配起始位置
    ///（顺序与构建 Matcher 时提供的模式相同）。
    pub fn find_first_positions(&self, haystack: &[u8]) -> Vec<Option<usize>> {
        let mut first: Vec<Option<usize>> = vec![None; self.patterns.len()];
        for m in self.ac.find_iter(haystack) {
            let id = m.value();
            if id < first.len() && first[id].is_none() {
                first[id] = Some(m.start());
            }
        }
        first
    }

    /// Expose number of patterns
    pub fn patterns_len(&self) -> usize {
        self.patterns.len()
    }
}
