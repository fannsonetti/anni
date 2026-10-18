/// Convert HTML content to readable plain text.
/// Strips tags, decodes entities, preserves paragraph breaks.
pub fn html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut last_was_newline = false;

    let html = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n");

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => {
                if ch == '\n' {
                    if !last_was_newline {
                        result.push('\n');
                    }
                    last_was_newline = true;
                } else {
                    last_was_newline = false;
                    result.push(ch);
                }
            }
            _ => {}
        }
    }

    decode_html_entities(&result).trim().to_string()
}

fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&aacute;", "á")
        .replace("&Aacute;", "Á")
        .replace("&eacute;", "é")
        .replace("&Eacute;", "É")
        .replace("&iacute;", "í")
        .replace("&Iacute;", "Í")
        .replace("&oacute;", "ó")
        .replace("&Oacute;", "Ó")
        .replace("&uacute;", "ú")
        .replace("&Uacute;", "Ú")
        .replace("&yacute;", "ý")
        .replace("&Yacute;", "Ý")
        .replace("&thorn;", "þ")
        .replace("&THORN;", "Þ")
        .replace("&eth;", "ð")
        .replace("&ETH;", "Ð")
        .replace("&aelig;", "æ")
        .replace("&AElig;", "Æ")
        .replace("&ouml;", "ö")
        .replace("&Ouml;", "Ö")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_html_strip() {
        assert_eq!(html_to_text("<p>Hello</p>"), "Hello");
    }

    #[test]
    fn test_icelandic_entities() {
        assert_eq!(html_to_text("&aacute;&thorn;&aelig;&Eacute;"), "áþæÉ");
    }

    #[test]
    fn test_nested_tags() {
        assert_eq!(
            html_to_text("<p><strong>Bold</strong> text</p>"),
            "Bold text"
        );
    }

    #[test]
    fn test_br_tags() {
        assert_eq!(html_to_text("Line1<br>Line2"), "Line1\nLine2");
    }
}
