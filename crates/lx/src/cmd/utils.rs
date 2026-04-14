/// 从 URL 或纯 ID 中解析出 `space_id`
///
/// 支持以下格式:
/// - 纯 `space_id`: `19f662af7b0c4cf7b3674928a1b2d805`
/// - 乐享 URL: `https://lexiangla.com/spaces/19f662af7b0c4cf7b3674928a1b2d805`
/// - 带查询参数: `https://lexiangla.com/spaces/19f662af7b0c4cf7b3674928a1b2d805?company_from=123`
/// - `lexiang.tencent.com`: `https://lexiang.tencent.com/spaces/19f662af7b0c4cf7b3674928a1b2d805`
///
/// 支持的域名: `*.lexiangla.com`, `lexiang.tencent.com`
pub fn parse_space_id(input: &str) -> String {
    // 尝试解析为 URL
    if let Ok(url) = url::Url::parse(input) {
        if is_lexiang_host(url.host_str().unwrap_or("")) {
            // 从路径中提取 space_id: /spaces/{space_id}
            let segments: Vec<&str> = url
                .path_segments()
                .map(std::iter::Iterator::collect)
                .unwrap_or_default();
            if segments.len() >= 2 && segments[0] == "spaces" {
                return segments[1].to_string();
            }
        }
    }

    // 不是 URL，原样返回（当作纯 space_id）
    input.to_string()
}

/// 判断是否是乐享域名
fn is_lexiang_host(host: &str) -> bool {
    host.ends_with(".lexiangla.com") || host == "lexiangla.com" || host == "lexiang.tencent.com"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_space_id() {
        assert_eq!(
            parse_space_id("19f662af7b0c4cf7b3674928a1b2d805"),
            "19f662af7b0c4cf7b3674928a1b2d805"
        );
    }

    #[test]
    fn test_lexiangla_url() {
        assert_eq!(
            parse_space_id("https://lexiangla.com/spaces/19f662af7b0c4cf7b3674928a1b2d805"),
            "19f662af7b0c4cf7b3674928a1b2d805"
        );
    }

    #[test]
    fn test_lexiangla_url_with_query() {
        assert_eq!(
            parse_space_id(
                "https://lexiangla.com/spaces/19f662af7b0c4cf7b3674928a1b2d805?company_from=234"
            ),
            "19f662af7b0c4cf7b3674928a1b2d805"
        );
    }

    #[test]
    fn test_tencent_url() {
        assert_eq!(
            parse_space_id("https://lexiang.tencent.com/spaces/19f662af7b0c4cf7b3674928a1b2d805"),
            "19f662af7b0c4cf7b3674928a1b2d805"
        );
    }

    #[test]
    fn test_tencent_url_with_query() {
        assert_eq!(
            parse_space_id("https://lexiang.tencent.com/spaces/abc123?foo=bar&baz=qux"),
            "abc123"
        );
    }

    #[test]
    fn test_unknown_url_returns_as_is() {
        let input = "https://example.com/spaces/abc123";
        assert_eq!(parse_space_id(input), input);
    }

    #[test]
    fn test_lexiangla_bare_domain() {
        assert_eq!(
            parse_space_id("https://lexiangla.com/spaces/19f662af7b0c4cf7b3674928a1b2d805"),
            "19f662af7b0c4cf7b3674928a1b2d805"
        );
    }
}
