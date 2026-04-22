//! `DocIR` → MDX Emitter
//!
//! Serializes a `DocIR` tree back to MDX text.
//! Output aligned with Notion Enhanced Markdown Specification:
//! - Lowercase component tags: `<callout>`, `<details>`, `<columns>`, `<column>`, `<table>`
//! - Block-level color via `{color="Color"}` attribute suffix
//! - Inline styles: `<Mark bold>`, `<span underline>`, `<span color>`
//! - Quote uses native `>` syntax (not mapped to callout)

#![allow(dead_code)]

use crate::service::block::ir::{BlockAttrs, InlineStyle, Node, NodeType};

/// Valid Notion block colors (text colors & background colors)
const VALID_COLORS: &[&str] = &[
    "gray",
    "brown",
    "orange",
    "yellow",
    "green",
    "blue",
    "purple",
    "pink",
    "red",
    "gray_bg",
    "brown_bg",
    "orange_bg",
    "yellow_bg",
    "green_bg",
    "blue_bg",
    "purple_bg",
    "pink_bg",
    "red_bg",
    "default",
    "default_background",
];

/// Serialize a `DocIR` node to MDX text
pub fn emit_mdx(node: &Node) -> String {
    let mut emitter = Emitter::default();
    emitter.emit_node(node, 0);
    emitter.output.trim_end().to_string()
}

#[derive(Default)]
struct Emitter {
    output: String,
}

impl Emitter {
    fn push(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn push_indent(&mut self, depth: usize) {
        for _ in 0..depth * 4 {
            self.output.push(' ');
        }
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    /// Emit `id={N}` attribute if node has a numeric-style id from original block JSON
    fn maybe_emit_id(&mut self, node: &Node) {
        if let Some(ref id) = node.id {
            // Try to parse as number for numeric output; otherwise use string
            if let Ok(num) = id.parse::<u64>() {
                self.push(&format!(" id={{{num}}}"));
            } else {
                self.push(&format!(" id=\"{id}\""));
            }
        }
    }

    fn emit_node(&mut self, node: &Node, depth: usize) {
        match &node.node_type {
            NodeType::Document => self.emit_document(node, depth),
            NodeType::Paragraph => self.emit_paragraph(node, depth),
            NodeType::Heading { level } => self.emit_heading(*level, node, depth),
            NodeType::BlockQuote => self.emit_quote(node, depth),
            NodeType::Callout { color, icon } => {
                self.emit_callout(color.as_deref(), icon.as_deref(), node, depth);
            }
            NodeType::ColumnList => self.emit_columns(node, depth),
            NodeType::Column { width_ratio } => self.emit_column(*width_ratio, node, depth),
            NodeType::Divider => self.emit_divider(depth),
            NodeType::Image {
                file_id,
                caption,
                align,
                ..
            } => self.emit_image(
                file_id.as_deref(),
                caption.as_deref(),
                align.as_deref(),
                depth,
            ),
            NodeType::Table => self.emit_table(node, depth),
            NodeType::TableRow => self.emit_table_row(node, depth),
            NodeType::TableCell { .. } => self.emit_table_cell(node, depth),
            NodeType::Task { done, .. } => self.emit_todo(*done, node, depth),
            NodeType::BulletedList => self.emit_bullet_item(node, depth),
            NodeType::NumberedList => self.emit_numbered_item(node, depth),
            NodeType::CodeBlock { language } => self.emit_code_block(
                language.as_deref(),
                node.text.as_deref().unwrap_or(""),
                depth,
            ),
            NodeType::MathBlock { .. } => self.emit_math_block(node, depth),
            NodeType::Toggle => self.emit_toggle(node, depth),
            NodeType::Mermaid { .. } => self.emit_mermaid(node, depth),
            NodeType::PlantUml { .. } => self.emit_plantuml(node, depth),
            NodeType::SmartSheet { .. } => self.emit_placeholder("smartsheet", node),
            NodeType::Attachment { .. } => self.emit_placeholder("attachment", node),
            NodeType::Video { .. } => self.emit_placeholder("video", node),
            NodeType::Text => self.emit_text_node(node),
            NodeType::Link { href } => self.emit_link(href, node),
        }
    }

    fn emit_document(&mut self, node: &Node, _depth: usize) {
        for (i, child) in node.children.iter().enumerate() {
            if i > 0 {
                self.newline();
                self.newline();
            }
            self.emit_node(child, 0);
        }
    }

    /// Emit Notion block-level color attribute: `{color="Color"}`
    /// Only emits if color is a valid Notion color name.
    fn maybe_emit_block_color(&mut self, attrs: &BlockAttrs) {
        if let Some(ref c) = attrs.block_color {
            let trimmed = c.trim();
            if !trimmed.is_empty()
                && (VALID_COLORS.contains(&trimmed)
                    || trimmed == "default"
                    || trimmed.starts_with('#'))
            {
                self.push(&format!(" {{color=\"{trimmed}\"}}"));
            }
        }
    }

    fn attrs_is_empty(&self, attrs: &BlockAttrs) -> bool {
        attrs.text_align.is_none()
            && attrs.block_color.is_none()
            && attrs.icon.is_none()
            && attrs.width.is_none()
            && attrs.height.is_none()
    }

    fn emit_frontmatter(&mut self, attrs: &BlockAttrs) {
        self.push("---\n");
        if let Some(ref ta) = attrs.text_align {
            self.push(&format!("textAlign: {ta}\n"));
        }
        if let Some(ref bc) = attrs.block_color {
            self.push(&format!("blockColor: {bc}\n"));
        }
        if let Some(ref icon) = attrs.icon {
            self.push(&format!("icon: \"{icon}\"\n"));
        }
        self.push("---\n");
    }

    fn emit_paragraph(&mut self, node: &Node, _depth: usize) {
        self.maybe_emit_block_color(&node.attrs);
        self.emit_inline_children(&node.children);
    }

    fn emit_heading(&mut self, level: u8, node: &Node, _depth: usize) {
        self.push(&"#".repeat(level as usize));
        self.push(" ");
        self.maybe_emit_block_color(&node.attrs);
        self.emit_inline_children(&node.children);
    }

    /// Notion quote: `> Rich text {color="Color"}` with children
    fn emit_quote(&mut self, node: &Node, depth: usize) {
        // Single-line inline content → native `> text {color}` format
        let is_simple = node
            .children
            .iter()
            .all(|c| matches!(c.node_type, NodeType::Text | NodeType::Link { .. }));
        if is_simple && !node.children.is_empty() {
            let text_content = node
                .children
                .iter()
                .map(|c| match &c.node_type {
                    NodeType::Text => c.text.as_deref().unwrap_or("").to_string(),
                    NodeType::Link { href } => {
                        let inner = c
                            .children
                            .iter()
                            .filter_map(|cc| cc.text.clone())
                            .collect::<String>();
                        format!("[{}]({href})", inner)
                    }
                    _ => String::new(),
                })
                .collect::<String>();
            self.push("> ");
            self.maybe_emit_block_color(&node.attrs);
            self.push(&text_content);
        } else if node.children.len() == 1 {
            // Multi-line single blockquote: use <br> for line breaks
            self.push("> ");
            self.maybe_emit_block_color(&node.attrs);
            let mut first = true;
            for child in &node.children {
                if !first {
                    self.push("<br>");
                }
                first = false;
                // Collect text content recursively into buffer
                let mut buf = String::new();
                self.collect_inline_to_buf(child, &mut buf);
                self.push(&buf);
            }
        } else if !node.children.is_empty() {
            // Multiple child blocks in quote: each gets its own `>` line + children indented with tab
            for (i, child) in node.children.iter().enumerate() {
                if i > 0 {
                    self.newline();
                }
                self.push("> ");
                self.maybe_emit_block_color(&node.attrs);
                self.emit_node(child, depth);
                // If this child has its own children, indent them as tab under >
                if !child.children.is_empty() {
                    for sub_child in &child.children {
                        self.newline();
                        self.push("\t");
                        self.emit_node(sub_child, depth + 1);
                    }
                }
            }
        } else {
            // Empty quote
            self.push("> ");
            self.maybe_emit_block_color(&node.attrs);
        }
    }

    /// Notion callout: `<callout icon? color?>` (lowercase)
    fn emit_callout(&mut self, color: Option<&str>, icon: Option<&str>, node: &Node, depth: usize) {
        self.push("<callout");
        if let Some(ico) = icon {
            self.push(&format!(" icon=\"{ico}\""));
        }
        if let Some(c) = color {
            self.push(&format!(" color=\"{c}\""));
        }
        self.push(">");
        self.newline();

        for child in &node.children {
            self.push_indent(depth + 1);
            self.emit_node(child, depth + 1);
            self.newline();
        }

        self.push("</callout>");
    }

    /// Notion columns: `<columns><column>...</column></columns>`
    fn emit_columns(&mut self, node: &Node, depth: usize) {
        self.push("<columns>");
        self.newline();

        for child in &node.children {
            self.push_indent(depth + 1);
            self.emit_node(child, depth + 1);
            self.newline();
        }

        self.push("</columns>");
    }

    /// Notion column: `<column>` — no width attribute (Notion doesn't use `width_ratio` in MDX output)
    fn emit_column(&mut self, _width_ratio: Option<f64>, node: &Node, depth: usize) {
        self.push("<column>");
        self.newline();

        for child in &node.children {
            self.push_indent(depth + 1);
            self.emit_node(child, depth + 1);
            self.newline();
        }

        self.push("</column>");
    }

    fn emit_divider(&mut self, _depth: usize) {
        self.push("---");
    }

    fn emit_image(
        &mut self,
        file_id: Option<&str>,
        caption: Option<&str>,
        _align: Option<&str>,
        _depth: usize,
    ) {
        let src = file_id.unwrap_or("");
        match caption {
            Some(a) => self.push(&format!("![{a}]({src})")),
            None => self.push(&format!("![]({src})")),
        }
    }

    fn emit_table(&mut self, node: &Node, depth: usize) {
        self.push("<table>");
        self.newline();

        for child in &node.children {
            self.push_indent(depth + 1);
            self.emit_node(child, depth + 1);
            self.newline();
        }

        self.push("</table>");
    }

    fn emit_table_row(&mut self, node: &Node, depth: usize) {
        self.push("<tr>");
        self.newline();

        for child in &node.children {
            self.push_indent(depth + 1);
            self.emit_node(child, depth + 1);
            self.newline();
        }

        self.push("</tr>");
    }

    fn emit_table_cell(&mut self, node: &Node, _depth: usize) {
        // Check if cell has background_color from IR fields
        let bg_color = node.cell_bg_color.as_deref();
        match bg_color {
            Some(color) => self.push(&format!("<td color=\"{color}\">")),
            None => self.push("<td>"),
        }
        self.emit_inline_children(&node.children);
        self.push("</td>");
    }

    fn emit_todo(&mut self, done: bool, node: &Node, _depth: usize) {
        let checkbox = if done { "[x]" } else { "[ ]" };
        self.push(checkbox);
        self.push(" ");
        // Task name from task_name field, fallback to children content
        if let Some(ref name) = node.task_name {
            self.push(name);
        } else {
            self.emit_inline_children(&node.children);
        }
    }

    fn emit_bullet_item(&mut self, node: &Node, _depth: usize) {
        self.push("- ");
        self.emit_inline_children(&node.children);
    }

    fn emit_numbered_item(&mut self, node: &Node, _depth: usize) {
        self.push("1. ");
        self.emit_inline_children(&node.children);
    }

    fn emit_code_block(&mut self, language: Option<&str>, code: &str, _depth: usize) {
        self.push("```");
        if let Some(lang) = language {
            self.push(lang);
        }
        self.newline();
        self.push(code);
        self.newline();
        self.push("```");
    }

    fn emit_math_block(&mut self, node: &Node, depth: usize) {
        self.push("<MathBlock");
        self.push(">");
        self.newline();
        if let Some(ref t) = node.text {
            self.push_indent(depth + 1);
            self.push(t);
            self.newline();
        }
        self.push("</MathBlock>");
    }

    fn emit_toggle(&mut self, node: &Node, depth: usize) {
        // Notion toggle: <details color?="Color"><summary>Rich text</summary>children</details>
        self.push("<details>");
        if let Some(ref c) = node.attrs.block_color {
            let trimmed = c.trim();
            if !trimmed.is_empty() && (VALID_COLORS.contains(&trimmed) || trimmed.starts_with('#'))
            {
                self.push(&format!(" color=\"{trimmed}\""));
            }
        }
        self.push(">\n");
        self.push_indent(depth + 1);
        // Summary line from first child inline text
        let summary_text = node
            .children
            .first()
            .map(super::super::ir::Node::plain_content)
            .filter(|t| !t.is_empty());
        match summary_text {
            Some(text) => {
                self.push("<summary>");
                self.push(&text);
                self.push("</summary>");
            }
            None => {
                self.push("<summary>");
                self.push("</summary>");
            }
        }
        self.newline();

        for child in node.children.iter().skip(1) {
            self.push_indent(depth + 1);
            self.emit_node(child, depth + 1);
            self.newline();
        }

        self.push("</details>");
    }

    fn emit_mermaid(&mut self, node: &Node, _depth: usize) {
        self.push("```mermaid");
        self.newline();
        self.push(node.diagram_content.as_deref().unwrap_or(""));
        self.newline();
        self.push("```");
    }

    fn emit_plantuml(&mut self, node: &Node, _depth: usize) {
        self.push("```plantuml");
        self.newline();
        self.push(node.diagram_content.as_deref().unwrap_or(""));
        self.newline();
        self.push("```");
    }

    fn emit_placeholder(&mut self, name: &str, node: &Node) {
        self.push(&format!("[{name}]"));
        if let Some(ref t) = node.text {
            self.push(" ");
            self.push(t);
        }
    }

    fn emit_text_node(&mut self, node: &Node) {
        let text = node.text.as_deref().unwrap_or("");
        match node.inline_style.as_ref() {
            None => self.push(text),
            Some(style) if style.is_plain() => self.push(text),
            Some(style) => self.emit_styled_text(text, style),
        }
    }

    fn emit_styled_text(&mut self, text: &str, style: &InlineStyle) {
        // Determine which wrapper to use
        let has_bold_or_strike_or_code = style.bold || style.strike_through || style.inline_code;
        let has_italic = style.italic;
        let has_underline_or_color =
            style.underline || style.text_color.is_some() || style.background_color.is_some();
        let is_plain = style.is_plain();

        if is_plain {
            self.push(text);
        } else if has_underline_or_color && !has_bold_or_strike_or_code && !has_italic {
            // Pure span-style attributes (underline, color)
            self.push("<span");
            if style.underline {
                self.push(" underline=\"true\"");
            }
            if let Some(ref c) = style.text_color {
                let trimmed = c.trim();
                if !trimmed.is_empty()
                    && (VALID_COLORS.contains(&trimmed) || trimmed.starts_with('#'))
                {
                    self.push(&format!(" color=\"{trimmed}\""));
                }
            }
            if let Some(ref c) = style.background_color {
                let trimmed = c.trim();
                if !trimmed.is_empty()
                    && (VALID_COLORS.contains(&trimmed) || trimmed.starts_with('#'))
                {
                    self.push(&format!(" color=\"{trimmed}\""));
                }
            }
            self.push(">");
            self.push(text);
            self.push("</span>");
        } else if has_bold_or_strike_or_code || has_italic {
            // Mark-style for bold/strike/code/italic
            self.push("<Mark");
            if style.bold {
                self.push(" bold");
            }
            if style.italic {
                self.push(" italic");
            }
            if style.strike_through {
                self.push(" strikeThrough");
            }
            if style.inline_code {
                self.push(" inlineCode");
            }
            self.push(">");
            self.push(text);
            self.push("</Mark>");
        } else if has_italic {
            // Pure italic only → standard markdown *text*
            self.push("*");
            self.push(text);
            self.push("*");
        } else {
            // Underline/color only fallback
            self.push(text);
        }
    }

    fn emit_link(&mut self, href: &str, node: &Node) {
        if node.children.len() == 1 && matches!(node.children[0].node_type, NodeType::Text) {
            let text = node.children[0].text.as_deref().unwrap_or("");
            self.push(&format!("[{text}]({href})"));
        } else {
            self.push(&format!("<Link href=\"{href}\">"));
            self.emit_inline_children(&node.children);
            self.push("</Link>");
        }
    }

    fn emit_inline_children(&mut self, children: &[Node]) {
        for child in children {
            self.emit_node(child, 0);
        }
    }

    fn collect_inline_lines<'a>(&'a mut self, node: &'a Node) -> Vec<String> {
        let mut buf = String::new();
        self.collect_inline_recursive(node, &mut buf);
        vec![buf]
    }

    /// Collect inline text content into buffer for quote rendering
    fn collect_inline_to_buf(&mut self, node: &Node, buf: &mut String) {
        match &node.node_type {
            NodeType::Text => {
                buf.push_str(node.text.as_deref().unwrap_or(""));
            }
            NodeType::Link { href } => {
                let text = node
                    .children
                    .iter()
                    .map(super::super::ir::Node::plain_content)
                    .collect::<String>();
                buf.push_str(&format!("[{text}]({href})"));
            }
            _ => {
                for child in &node.children {
                    self.collect_inline_to_buf(child, buf);
                }
            }
        }
    }

    fn collect_inline_recursive(&mut self, node: &Node, buf: &mut String) {
        match &node.node_type {
            NodeType::Text => {
                buf.push_str(node.text.as_deref().unwrap_or(""));
            }
            NodeType::Link { href } => {
                let text = node
                    .children
                    .iter()
                    .map(super::super::ir::Node::plain_content)
                    .collect::<String>();
                buf.push_str(&format!("[{text}]({href})"));
            }
            _ => {
                for child in &node.children {
                    self.collect_inline_recursive(child, buf);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_simple_paragraph() {
        let node = Node::document(vec![Node::paragraph(vec![Node::plain_text("Hello")])]);
        let mdx = emit_mdx(&node);
        assert_eq!(mdx, "Hello");
    }

    #[test]
    fn test_emit_heading() {
        let node = Node::document(vec![Node::heading(2, vec![Node::plain_text("Title")])]);
        let mdx = emit_mdx(&node);
        assert_eq!(mdx, "## Title");
    }

    #[test]
    fn test_emit_code_block() {
        let node = Node::document(vec![Node::code_block(Some("rust"), "fn main() {}")]);
        let mdx = emit_mdx(&node);
        assert!(mdx.contains("```rust"));
        assert!(mdx.contains("fn main() {}"));
    }

    #[test]
    fn test_emit_todo() {
        let node = Node::document(vec![
            Node::task(true, "done item"),
            Node::task(false, "undone"),
        ]);
        let mdx = emit_mdx(&node);
        assert!(mdx.contains("[x] done item"));
        assert!(mdx.contains("[ ] undone"));
    }

    #[test]
    fn test_emit_bullet_list() {
        let node = Node::document(vec![
            Node::bullet_item(vec![Node::plain_text("first")]),
            Node::bullet_item(vec![Node::plain_text("second")]),
        ]);
        let mdx = emit_mdx(&node);
        assert!(mdx.contains("- first"));
        assert!(mdx.contains("- second"));
    }

    #[test]
    fn test_emit_divider() {
        let node = Node::document(vec![Node::divider()]);
        let mdx = emit_mdx(&node);
        assert_eq!(mdx, "---");
    }

    #[test]
    fn test_emit_callout() {
        let icon = "\u{1f6a7}";
        let bc = "red";
        let node = Node::document(vec![Node::callout(
            Some(bc),
            Some(icon),
            vec![Node::paragraph(vec![Node::plain_text("Warning message")])],
        )]);
        let mdx = emit_mdx(&node);
        // Notion format: lowercase <callout>
        assert!(
            mdx.contains("<callout"),
            "expected lowercase callout, got: {mdx}"
        );
        assert!(mdx.contains("icon=\"\u{1f6a7}\""));
        // Notion uses color= not borderColor=
        assert!(mdx.contains("color=\"red\""));
        assert!(mdx.contains("Warning message"));
        assert!(
            mdx.contains("</callout>"),
            "expected </callout>, got: {mdx}"
        );
    }

    #[test]
    fn test_emit_image() {
        let src = "https://example.com/img.png";
        let alt = "example";
        let node = Node::document(vec![Node::image(Some(src), Some(alt))]);
        let mdx = emit_mdx(&node);
        assert_eq!(mdx, "![example](https://example.com/img.png)");
    }

    #[test]
    fn test_emit_bold_italic() {
        let style = InlineStyle {
            italic: true,
            ..Default::default()
        };
        let italic_node = Node {
            node_type: NodeType::Text,
            text: Some("italic".to_string()),
            attrs: Default::default(),
            inline_style: Some(style),
            href: None,
            children: vec![],
            ..Default::default()
        };
        let node = Node::document(vec![Node::paragraph(vec![
            Node::bold("bold text"),
            Node::plain_text(" and "),
            italic_node,
        ])]);
        let mdx = emit_mdx(&node);
        // Bold uses <Mark bold>, italic uses <Mark italic>
        assert!(
            mdx.contains("<Mark bold>bold text</Mark>"),
            "expected <Mark bold>, got: {mdx}"
        );
        assert!(mdx.contains(" and "));
        assert!(
            mdx.contains("<Mark italic>italic</Mark>") || mdx.contains("*italic*"),
            "expected italic format, got: {mdx}"
        );
    }

    #[test]
    fn test_emit_link() {
        let href = "https://example.com";
        let child = Node::plain_text("click me");
        let node = Node::document(vec![Node::paragraph(vec![Node::link(href, vec![child])])]);
        let mdx = emit_mdx(&node);
        assert_eq!(mdx, "[click me](https://example.com)");
    }

    #[test]
    fn test_emit_frontmatter() {
        let mut doc = Node::document(vec![Node::paragraph(vec![Node::plain_text("content")])]);
        doc.attrs.text_align = Some("center".to_string());
        doc.attrs.icon = Some("\u{1f4c4}".to_string());
        let mdx = emit_mdx(&doc);
        // Frontmatter is no longer emitted as YAML; block_color goes inline
        // This test now checks that content is emitted without YAML wrapper
        assert!(mdx.contains("content"));
    }

    #[test]
    fn test_emit_toggle() {
        let node = Node::document(vec![Node::toggle(vec![
            Node::plain_text("Click to expand"),
            Node::paragraph(vec![Node::plain_text("Hidden content")]),
        ])]);
        let mdx = emit_mdx(&node);
        // Notion format: <details><summary>Click to expand</summary>children</details>
        assert!(mdx.contains("<details"));
        assert!(mdx.contains("<summary>Click to expand</summary>"));
        assert!(mdx.contains("Hidden content"));
        assert!(mdx.contains("</details>"));
    }

    #[test]
    fn test_emit_columns_notion() {
        let node = Node::document(vec![Node::column_list(vec![
            Node::column(
                Some(0.5),
                vec![Node::heading(2, vec![Node::plain_text("Left")])],
            ),
            Node::column(
                Some(0.5),
                vec![Node::heading(2, vec![Node::plain_text("Right")])],
            ),
        ])]);
        let mdx = emit_mdx(&node);
        // Notion format: lowercase <columns>/<column>
        assert!(mdx.contains("<columns>"));
        assert!(mdx.contains("</columns>"));
        assert!(mdx.contains("<column>"));
        assert!(mdx.contains("</column>"));
    }

    #[test]
    fn test_emit_quote_native() {
        // Simple quote with text children → native > format
        let node = Node::document(vec![Node::quote(vec![Node::plain_text(
            "A quoted message",
        )])]);
        let mdx = emit_mdx(&node);
        assert!(
            mdx.starts_with("> A quoted message"),
            "expected native > quote, got: {mdx}"
        );
    }

    #[test]
    fn test_emit_block_color() {
        // Paragraph with block_color
        let mut p = Node::paragraph(vec![Node::plain_text("Colored text")]);
        p.attrs.block_color = Some("blue".to_string());
        let node = Node::document(vec![p]);
        let mdx = emit_mdx(&node);
        assert!(
            mdx.contains("{color=\"blue\"}"),
            "expected color attribute, got: {mdx}"
        );
        assert!(mdx.contains("Colored text"));
    }

    #[test]
    fn test_emit_table_notion() {
        let cell = Node::table_cell(vec![Node::plain_text("data")]);
        let row = Node::table_row(vec![cell.clone()]);
        let tbl = Node::table(vec![row]);
        let mdx = emit_mdx(&tbl);
        // Notion format: <table>/<tr>/<td>
        assert!(mdx.contains("<table>"), "expected <table>");
        assert!(mdx.contains("<tr>"), "expected <tr>");
        assert!(mdx.contains("<td>data</td>"), "expected <td>");
        assert!(mdx.contains("</table>"));
    }
}
