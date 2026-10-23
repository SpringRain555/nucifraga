//! 對 JSON 原文做最小替換（ADR-0006）。
//!
//! 模板檔可能是使用者在別處維護的檔案，別的工具也在讀它。所以寫回時**只動目標值的位元組**：
//! 縮排、欄位順序、行尾、其他欄位一律不碰。做法是自己掃一遍原文，記下每個值的位元組範圍，
//! 而不是「讀成物件再序列化」——那會把整份檔案重排。

use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub enum EditError {
    /// 原文不是合法 JSON（呼叫端應該先用 serde_json 驗過，這裡只是保險）。
    Malformed(usize),
    /// 路徑上某一層存在，但不是物件。
    NotAnObject(String),
}

/// 把 `path`（一串物件 key）指到的值換成 `new_value`（已經序列化好的 JSON 片段）。
/// 中間缺的物件會自動補上；缺的 key 加在那個物件的最後一個成員後面，沿用既有成員的縮排。
pub fn set_value(text: &str, path: &[&str], new_value: &str) -> Result<String, EditError> {
    assert!(!path.is_empty(), "path 不可為空");
    let bytes = text.as_bytes();
    let root_start = skip_ws(bytes, 0);
    let root = parse_value(bytes, root_start)?;
    let mut obj = root;
    for (depth, key) in path.iter().enumerate() {
        if bytes.get(obj.start) != Some(&b'{') {
            return Err(EditError::NotAnObject(path[..depth].join(".")));
        }
        let members = object_members(bytes, obj.clone())?;
        match members.iter().find(|m| &text[m.key.clone()] == *key) {
            Some(m) => obj = m.value.clone(),
            None => {
                let value = nested_value(&path[depth + 1..], new_value);
                return Ok(insert_member(text, obj, &members, key, &value));
            }
        }
    }
    let mut out = String::with_capacity(text.len() + new_value.len());
    out.push_str(&text[..obj.start]);
    out.push_str(new_value);
    out.push_str(&text[obj.end..]);
    Ok(out)
}

/// `["a","b"]` 與值 `1` → `{"a": {"b": 1}}`（給缺了中間物件的情況）。
fn nested_value(rest: &[&str], leaf: &str) -> String {
    rest.iter().rev().fold(leaf.to_string(), |acc, key| {
        format!("{{ \"{key}\": {acc} }}")
    })
}

struct Member {
    key: Range<usize>,
    value: Range<usize>,
    /// 這個成員那一行的縮排（key 前面的空白）。
    indent: String,
}

fn insert_member(
    text: &str,
    obj: Range<usize>,
    members: &[Member],
    key: &str,
    value: &str,
) -> String {
    let close = obj.end - 1; // '}' 的位置
    let mut out = String::with_capacity(text.len() + key.len() + value.len() + 16);
    match members.last() {
        Some(last) => {
            let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let same_line = !text[last.value.end..close].contains('\n')
                && !text[obj.start..last.key.start].contains('\n');
            out.push_str(&text[..last.value.end]);
            if same_line {
                out.push_str(&format!(", \"{key}\": {value}"));
            } else {
                out.push_str(&format!(",{newline}{}\"{key}\": {value}", last.indent));
            }
            out.push_str(&text[last.value.end..]);
        }
        None => {
            out.push_str(&text[..obj.start]);
            out.push_str(&format!("{{ \"{key}\": {value} }}"));
            out.push_str(&text[obj.end..]);
        }
    }
    out
}

fn object_members(b: &[u8], obj: Range<usize>) -> Result<Vec<Member>, EditError> {
    let mut members = Vec::new();
    let mut i = skip_ws(b, obj.start + 1);
    if b.get(i) == Some(&b'}') {
        return Ok(members);
    }
    loop {
        let key = parse_string(b, i)?;
        let line_start = b[..key.start]
            .iter()
            .rposition(|&c| c == b'\n')
            .map_or(0, |p| p + 1);
        let indent: String = b[line_start..key.start]
            .iter()
            .take_while(|c| **c == b' ' || **c == b'\t')
            .map(|&c| c as char)
            .collect();
        i = skip_ws(b, key.end);
        if b.get(i) != Some(&b':') {
            return Err(EditError::Malformed(i));
        }
        i = skip_ws(b, i + 1);
        let value = parse_value(b, i)?;
        i = skip_ws(b, value.end);
        members.push(Member {
            key: key.start + 1..key.end - 1,
            value,
            indent,
        });
        match b.get(i) {
            Some(b',') => i = skip_ws(b, i + 1),
            Some(b'}') => return Ok(members),
            _ => return Err(EditError::Malformed(i)),
        }
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    i
}

/// 回傳含引號的範圍。
fn parse_string(b: &[u8], start: usize) -> Result<Range<usize>, EditError> {
    if b.get(start) != Some(&b'"') {
        return Err(EditError::Malformed(start));
    }
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Ok(start..i + 1),
            _ => i += 1,
        }
    }
    Err(EditError::Malformed(start))
}

fn parse_value(b: &[u8], start: usize) -> Result<Range<usize>, EditError> {
    match b.get(start) {
        Some(b'"') => parse_string(b, start),
        Some(b'{') | Some(b'[') => {
            let (open, close) = if b[start] == b'{' {
                (b'{', b'}')
            } else {
                (b'[', b']')
            };
            let mut depth = 0usize;
            let mut i = start;
            while i < b.len() {
                match b[i] {
                    b'"' => {
                        i = parse_string(b, i)?.end;
                        continue;
                    }
                    c if c == open => depth += 1,
                    c if c == close => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(start..i + 1);
                        }
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            Err(EditError::Malformed(start))
        }
        Some(_) => {
            let mut i = start;
            while i < b.len() && !matches!(b[i], b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n')
            {
                i += 1;
            }
            if i == start {
                Err(EditError::Malformed(start))
            } else {
                Ok(start..i)
            }
        }
        None => Err(EditError::Malformed(start)),
    }
}

/// 把字串序列化成 JSON 字串字面值（含引號與跳脫）。
pub fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("字串一定序列化得了")
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: &str = "{\r\n  \"schemaVersion\": 1,\r\n  \"destination\": {\r\n    \"root\": \"E:\\\\Backup\",\r\n    \"snapshot\": \"{yyyyMMdd}_backup\"\r\n  },\r\n  \"roots\": [\r\n    { \"path\": \"D:\\\\Obsidian\", \"pathCheck\": true }\r\n  ],\r\n  \"excludeDirNames\": [\r\n    \"node_modules\"\r\n  ]\r\n}\r\n";

    #[test]
    fn replacing_an_existing_value_touches_only_its_bytes() {
        let src = POLICY.replace(
            "\"snapshot\"",
            "\"nucifraga\": { \"destinationId\": null },\r\n    \"snapshot\"",
        );
        let out = set_value(
            &src,
            &["destination", "nucifraga", "destinationId"],
            "\"abc\"",
        )
        .unwrap();
        assert_eq!(
            out,
            src.replace("\"destinationId\": null", "\"destinationId\": \"abc\"")
        );
    }

    #[test]
    fn missing_object_is_appended_after_the_last_member_with_its_indent_and_crlf() {
        let out = set_value(POLICY, &["nucifraga", "destinationId"], "\"abc\"").unwrap();
        let expected = POLICY.replace(
            "    \"node_modules\"\r\n  ]\r\n}",
            "    \"node_modules\"\r\n  ],\r\n  \"nucifraga\": { \"destinationId\": \"abc\" }\r\n}",
        );
        assert_eq!(out, expected);
        // 除了插入的那一段，其餘位元組完全不變
        assert!(out.starts_with(&POLICY[..POLICY.len() - 5]));
        serde_json::from_str::<serde_json::Value>(&out).unwrap();
    }

    #[test]
    fn missing_key_in_an_existing_object_is_added_there() {
        let src = "{\n  \"nucifraga\": {\n    \"title\": \"x\"\n  }\n}\n";
        let out = set_value(src, &["nucifraga", "destinationId"], "\"id\"").unwrap();
        assert_eq!(
            out,
            "{\n  \"nucifraga\": {\n    \"title\": \"x\",\n    \"destinationId\": \"id\"\n  }\n}\n"
        );
    }

    #[test]
    fn single_line_objects_stay_on_one_line() {
        let out = set_value("{\"a\": 1}", &["b"], "2").unwrap();
        assert_eq!(out, "{\"a\": 1, \"b\": 2}");
    }

    #[test]
    fn empty_object_gets_the_member() {
        let out = set_value("{\"n\": {}}", &["n", "k"], "true").unwrap();
        assert_eq!(out, "{\"n\": { \"k\": true }}");
    }

    #[test]
    fn strings_containing_braces_and_escaped_quotes_are_skipped_correctly() {
        let src = r#"{"a": "x}\"{", "b": [1, {"c": "]"}], "d": 0}"#;
        let out = set_value(src, &["d"], "5").unwrap();
        assert_eq!(out, r#"{"a": "x}\"{", "b": [1, {"c": "]"}], "d": 5}"#);
    }

    #[test]
    fn replacing_an_array_value_works() {
        let out = set_value(r#"{"r": [1, 2], "z": 0}"#, &["r"], "[3]").unwrap();
        assert_eq!(out, r#"{"r": [3], "z": 0}"#);
    }

    #[test]
    fn walking_into_a_non_object_is_an_error() {
        assert_eq!(
            set_value(r#"{"a": 1}"#, &["a", "b"], "2"),
            Err(EditError::NotAnObject("a".into()))
        );
    }
}
