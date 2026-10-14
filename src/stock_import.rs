//! Conservative, bounded text import for the local stock catalog.
//!
//! Matching is by whole token, never by a substring of prose or a quote value.
//! Unknown security-shaped identifiers are retained for the import preview;
//! importing text does not add securities to the market-data catalog.

use std::{collections::HashSet, fmt};

use crate::market::Symbol;

pub const MAX_INPUT_BYTES: usize = 256 * 1024;
const MAX_CANDIDATES: usize = 5_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportPreview {
    /// Unique catalog matches, in their first appearance order.
    pub recognized: Vec<Symbol>,
    /// Recognized securities that are absent from this component's group.
    pub additions: Vec<Symbol>,
    pub already_present: usize,
    /// Repeated identifiers in the text, including repeated unknown codes.
    /// An adjacent code/name pair in a table row counts as one occurrence.
    pub duplicates: usize,
    pub unknown: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    InputTooLarge,
    TooManyCandidates,
    InvalidEncoding,
    InvalidText,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InputTooLarge => "文本不能超过 256 KiB，请分批导入",
            Self::TooManyCandidates => "一次最多识别 5000 条股票，请分批导入",
            Self::InvalidEncoding => "无法解码文本，请使用 UTF-8、UTF-16 或 GB18030 编码",
            Self::InvalidText => "内容包含非文本控制字符，请选择文本文件",
        })
    }
}

impl std::error::Error for ImportError {}

/// Decode common text exports without silently replacing malformed bytes.
pub fn decode_text(bytes: &[u8]) -> Result<String, ImportError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(ImportError::InputTooLarge);
    }
    let decoded = if let Some(body) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        encoding_rs::UTF_8.decode_without_bom_handling_and_without_replacement(body)
    } else if let Some(body) = bytes.strip_prefix(&[0xff, 0xfe]) {
        encoding_rs::UTF_16LE.decode_without_bom_handling_and_without_replacement(body)
    } else if let Some(body) = bytes.strip_prefix(&[0xfe, 0xff]) {
        encoding_rs::UTF_16BE.decode_without_bom_handling_and_without_replacement(body)
    } else {
        encoding_rs::UTF_8
            .decode_without_bom_handling_and_without_replacement(bytes)
            .or_else(|| {
                encoding_rs::GB18030.decode_without_bom_handling_and_without_replacement(bytes)
            })
    }
    .ok_or(ImportError::InvalidEncoding)?;
    validate_text(&decoded)?;
    Ok(decoded.into_owned())
}

fn validate_text(text: &str) -> Result<(), ImportError> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(ImportError::InputTooLarge);
    }
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(ImportError::InvalidText);
    }
    Ok(())
}

/// Recognize a pasted list or copied table, then exclude existing members.
///
/// Naked five/six-digit numbers are stock candidates. After a row's first
/// unambiguous quote value, unknown naked numbers are treated as quote values.
/// Known codes and explicit market qualifiers always remain identifiable.
/// A newline or semicolon starts a fresh record; commas can separate CSV fields.
pub fn parse(text: &str, existing: &[Symbol]) -> Result<ImportPreview, ImportError> {
    validate_text(text)?;
    let normalized: String = text
        .chars()
        .map(|c| match c {
            '\u{ff01}'..='\u{ff5e}' => char::from_u32(c as u32 - 0xfee0).unwrap(),
            '\u{3000}' => ' ',
            _ => c,
        })
        .collect();
    let mut preview = ImportPreview::default();
    let mut seen = HashSet::new();
    let mut count = 0;

    for line in normalized.split(['\n', '\r', ';']) {
        let mut in_values = false;
        let mut row_has_identifier = false;
        let mut pair: Option<(Symbol, bool)> = None;
        for token in line.split(|c: char| {
            !c.is_alphanumeric() && !matches!(c, '.' | ':' | '/' | '-' | '+' | '%' | '_')
        }) {
            // A colon also commonly separates prose from a code. Market
            // qualifiers and numeric clock values must remain indivisible.
            let parts: Vec<&str> =
                if token.contains(':') && !has_market_prefix(token) && !is_numeric_value(token) {
                    token.split(':').collect()
                } else {
                    vec![token]
                };
            for raw in parts {
                let raw = if raw.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                    raw
                } else {
                    raw.trim_end_matches('.')
                };
                if raw.is_empty() {
                    continue;
                }
                let naked_number = raw.bytes().all(|b| b.is_ascii_digit());
                let candidate = identify(raw);
                if naked_number
                    && in_values
                    && candidate.as_ref().is_none_or(|item| item.symbol.is_none())
                {
                    continue;
                }
                let Some(candidate) = candidate else {
                    if row_has_identifier && is_numeric_value(raw) {
                        in_values = true;
                    }
                    continue;
                };
                row_has_identifier = true;

                if let Some(symbol) = candidate.symbol {
                    if pair == Some((symbol, !candidate.is_name)) {
                        // Code + matching display name is a common export row.
                        pair = None;
                        continue;
                    }
                    pair = Some((symbol, candidate.is_name));
                } else {
                    pair = None;
                }
                count += 1;
                if count > MAX_CANDIDATES {
                    return Err(ImportError::TooManyCandidates);
                }
                if !seen.insert(candidate.key.clone()) {
                    preview.duplicates += 1;
                    continue;
                }
                if let Some(symbol) = candidate.symbol {
                    preview.recognized.push(symbol);
                    if existing.contains(&symbol) {
                        preview.already_present += 1;
                    } else {
                        preview.additions.push(symbol);
                    }
                } else {
                    preview.unknown.push(candidate.key);
                }
            }
        }
    }
    Ok(preview)
}

struct Candidate {
    key: String,
    symbol: Option<Symbol>,
    is_name: bool,
}

fn known(symbol: Symbol, is_name: bool) -> Candidate {
    Candidate {
        key: format!("{}:{}", symbol.market(), symbol.code()),
        symbol: Some(symbol),
        is_name,
    }
}

fn identify(raw: &str) -> Option<Candidate> {
    let upper = raw.to_ascii_uppercase();
    for symbol in Symbol::ALL {
        if upper == symbol.code() {
            return Some(known(symbol, false));
        }
        if raw == symbol.name()
            || aliases(symbol)
                .iter()
                .any(|name| raw.eq_ignore_ascii_case(name))
        {
            return Some(known(symbol, true));
        }
    }

    if let Some((market, code)) = qualified_code(&upper) {
        if code.is_empty() || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.') {
            return None;
        }
        let code = if market == "HK" && code.len() <= 5 && code.bytes().all(|b| b.is_ascii_digit())
        {
            format!("{code:0>5}")
        } else {
            code.to_string()
        };
        let symbol = Symbol::ALL
            .into_iter()
            .find(|symbol| symbol.market() == market && symbol.code() == code);
        return Some(Candidate {
            key: format!("{market}:{code}"),
            symbol,
            is_name: false,
        });
    }

    if raw.bytes().all(|b| b.is_ascii_digit()) {
        let key = match raw.len() {
            5 => format!("HK:{raw}"),
            6 => match &raw[..2] {
                "60" | "68" | "90" => format!("SH:{raw}"),
                "00" | "30" => format!("SZ:{raw}"),
                "43" | "83" | "87" | "88" | "92" => format!("BJ:{raw}"),
                _ => raw.to_string(),
            },
            // 通达信文本导出的市场前缀：0 深圳、1 上海。
            7 if raw.starts_with('0') => format!("SZ:{}", &raw[1..]),
            7 if raw.starts_with('1') => format!("SH:{}", &raw[1..]),
            _ => return None,
        };
        return Some(Candidate {
            key,
            symbol: None,
            is_name: false,
        });
    }

    // Unknown bare English prose is not assumed to be a ticker. Uppercase
    // ticker-shaped words can be reviewed; explicit US qualifiers accept case.
    if raw == upper && is_ticker(raw) && !is_header(raw) {
        return Some(Candidate {
            key: format!("US:{raw}"),
            symbol: None,
            is_name: false,
        });
    }
    None
}

fn aliases(symbol: Symbol) -> &'static [&'static str] {
    match symbol {
        Symbol::Aapl => &["苹果公司", "Apple"],
        Symbol::Msft => &["微软公司", "Microsoft"],
        Symbol::Nvda => &["Nvidia"],
        Symbol::Tsla => &["Tesla"],
        Symbol::Tencent => &["腾讯", "腾讯控股有限公司", "Tencent"],
        Symbol::Alibaba => &["阿里", "阿里巴巴集团", "阿里巴巴-SW", "Alibaba"],
    }
}

fn has_market_prefix(token: &str) -> bool {
    token.split_once(':').is_some_and(|(prefix, _)| {
        matches!(
            prefix.to_ascii_uppercase().as_str(),
            "US" | "HK" | "SH" | "SZ" | "BJ"
        )
    })
}

fn qualified_code(upper: &str) -> Option<(&str, &str)> {
    for market in ["US", "HK", "SH", "SZ", "BJ"] {
        if let Some(code) = upper.strip_suffix(&format!(".{market}")) {
            return Some((market, code));
        }
        if let Some(code) = upper.strip_prefix(&format!("{market}:")) {
            return Some((market, code));
        }
        if let Some(code) = upper.strip_prefix(market)
            && code.as_bytes().first().is_some_and(u8::is_ascii_digit)
        {
            return Some((market, code));
        }
    }
    None
}

fn is_numeric_value(token: &str) -> bool {
    token.bytes().any(|b| b.is_ascii_digit())
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-' | b'%' | b'/' | b':'))
}

fn is_ticker(token: &str) -> bool {
    let (base, class) = token.split_once('.').unwrap_or((token, ""));
    (1..=5).contains(&base.len())
        && base.bytes().all(|b| b.is_ascii_uppercase())
        && (class.is_empty() || (class.len() == 1 && class.bytes().all(|b| b.is_ascii_uppercase())))
}

fn is_header(token: &str) -> bool {
    matches!(
        token,
        "CODE"
            | "NAME"
            | "OPEN"
            | "CLOSE"
            | "HIGH"
            | "LOW"
            | "PRICE"
            | "DATE"
            | "TIME"
            | "TOTAL"
            | "VALUE"
            | "USD"
            | "HKD"
            | "US"
            | "HK"
            | "SH"
            | "SZ"
            | "BJ"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_lists_match_whole_codes_aliases_and_market_qualifiers_in_order() {
        let result = parse(
            "观察：aapl，US:msft; NVDA.US\t特斯拉\n700.HK、HK:9988\n腾讯控股；苹果公司\nＡＡＰＬ",
            &[Symbol::Msft],
        )
        .unwrap();
        assert_eq!(result.recognized, Symbol::ALL);
        assert_eq!(
            result.additions,
            [
                Symbol::Aapl,
                Symbol::Nvda,
                Symbol::Tsla,
                Symbol::Tencent,
                Symbol::Alibaba
            ]
        );
        assert_eq!(result.already_present, 1);
        assert_eq!(result.duplicates, 3);
        assert!(result.unknown.is_empty());
    }

    #[test]
    fn copied_table_pairs_names_with_codes_and_ignores_quote_columns() {
        let result = parse(
            "CODE\tNAME\tPRICE\t涨跌幅\t成交量\r\nAAPL\t苹果\t340.30\t-0.28%\t123456\r\n00700\t腾讯控股\t645\t1.23%\t10000\nMSFT 微软 10000.00 600519\nAAPL 苹果 123.45 00700",
            &[Symbol::Aapl],
        )
        .unwrap();
        assert_eq!(
            result.recognized,
            [Symbol::Aapl, Symbol::Tencent, Symbol::Msft]
        );
        assert_eq!(result.already_present, 1);
        assert_eq!(result.duplicates, 2);
        assert!(result.unknown.is_empty());
    }

    #[test]
    fn code_name_pairs_do_not_hide_later_codes_in_mixed_lists() {
        for text in [
            "AAPL 苹果,600519,00700",
            "AAPL 苹果;600519;00700",
            "AAPL 苹果 600519 00700",
        ] {
            let result = parse(text, &[]).unwrap();
            assert_eq!(result.recognized, [Symbol::Aapl, Symbol::Tencent]);
            assert_eq!(result.unknown, ["SH:600519"]);
            assert_eq!(result.duplicates, 0);
        }
        let result = parse("AAPL 苹果,MSFT,600519", &[]).unwrap();
        assert_eq!(result.recognized, [Symbol::Aapl, Symbol::Msft]);
        assert_eq!(result.unknown, ["SH:600519"]);
    }

    #[test]
    fn known_codes_survive_quote_columns_but_unknown_integers_need_clear_boundaries() {
        let result = parse("AAPL,340.30,00700,09988,700,600519", &[]).unwrap();
        assert_eq!(
            result.recognized,
            [Symbol::Aapl, Symbol::Tencent, Symbol::Alibaba]
        );
        assert!(result.unknown.is_empty());

        let result = parse("AAPL,340.30,SH600519;000001\n01234", &[]).unwrap();
        assert_eq!(result.recognized, [Symbol::Aapl]);
        assert_eq!(result.unknown, ["SH:600519", "SZ:000001", "HK:01234"]);
    }

    #[test]
    fn unknown_identifiers_are_visible_deduplicated_and_market_sensitive() {
        let result = parse(
            "SH00700 00700.SH HK00700 600519 SH600519 600519.SH 1600519\n0000001 000001.SZ GOOG US:goog 01234.HK HK01234",
            &[],
        )
        .unwrap();
        assert_eq!(result.recognized, [Symbol::Tencent]);
        assert_eq!(
            result.unknown,
            ["SH:00700", "SH:600519", "SZ:000001", "US:GOOG", "HK:01234"]
        );
        assert_eq!(result.duplicates, 7);
    }

    #[test]
    fn dates_prices_embedded_names_and_prose_are_not_substring_matches() {
        let result = parse(
            "2026/10/14 2026-10-14 20261014 09:30 340.30 +12.34% -00700 00700. 700 9988\n看好苹果 我的AAPL组合 AAPL2 APPLEx hello applepie MSFT_old \n股票:AAPL.US; 腾讯",
            &[],
        )
        .unwrap();
        assert_eq!(result.recognized, [Symbol::Aapl, Symbol::Tencent]);
        assert!(result.unknown.is_empty());
    }

    #[test]
    fn repeats_and_existing_members_are_separate_and_empty_input_is_valid() {
        let result = parse("AAPL AAPL\n苹果\nMSFT MSFT\nGOOG GOOG", &[Symbol::Aapl]).unwrap();
        assert_eq!(result.recognized, [Symbol::Aapl, Symbol::Msft]);
        assert_eq!(result.additions, [Symbol::Msft]);
        assert_eq!(result.already_present, 1);
        assert_eq!(result.duplicates, 4);
        assert_eq!(result.unknown, ["US:GOOG"]);
        assert_eq!(parse(" \n\t", &[]).unwrap(), ImportPreview::default());
    }

    #[test]
    fn oversized_text_or_candidate_lists_fail_without_partial_import() {
        assert_eq!(
            parse(&" ".repeat(MAX_INPUT_BYTES + 1), &[]),
            Err(ImportError::InputTooLarge)
        );
        assert_eq!(
            parse(&"AAPL\n".repeat(MAX_CANDIDATES + 1), &[]),
            Err(ImportError::TooManyCandidates)
        );
        assert!(parse(&" ".repeat(MAX_INPUT_BYTES), &[]).is_ok());
        assert!(parse(&"AAPL\n".repeat(MAX_CANDIDATES), &[]).is_ok());
        assert!(parse(&"AAPL 苹果\n".repeat(MAX_CANDIDATES), &[]).is_ok());
        assert_eq!(parse("AAPL\0MSFT", &[]), Err(ImportError::InvalidText));
    }

    #[test]
    fn file_decoding_accepts_utf8_utf16_and_legacy_chinese_exports() {
        let text = "AAPL 苹果\r\n00700 腾讯";
        assert_eq!(decode_text(text.as_bytes()).unwrap(), text);
        assert_eq!(
            decode_text(&[b"\xef\xbb\xbf".as_slice(), text.as_bytes()].concat()).unwrap(),
            text
        );
        for little_endian in [true, false] {
            let mut bytes = if little_endian {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for word in text.encode_utf16() {
                bytes.extend(if little_endian {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
            assert_eq!(decode_text(&bytes).unwrap(), text);
        }
        let (bytes, _, errors) = encoding_rs::GB18030.encode(text);
        assert!(!errors);
        assert_eq!(decode_text(&bytes).unwrap(), text);
    }

    #[test]
    fn decoding_rejects_malformed_encoding_binary_controls_and_large_files() {
        assert_eq!(decode_text(&[0xff]), Err(ImportError::InvalidEncoding));
        assert_eq!(
            decode_text(&[0xff, 0xfe, 0x00]),
            Err(ImportError::InvalidEncoding)
        );
        assert_eq!(
            decode_text(&[0xef, 0xbb, 0xbf, 0xff]),
            Err(ImportError::InvalidEncoding)
        );
        assert_eq!(decode_text(b"AAPL\0MSFT"), Err(ImportError::InvalidText));
        assert_eq!(decode_text(b"AAPL\x01MSFT"), Err(ImportError::InvalidText));
        assert_eq!(
            decode_text(&vec![b'a'; MAX_INPUT_BYTES + 1]),
            Err(ImportError::InputTooLarge)
        );
    }
}
