//! Minimal Markdown preview using pulldown-cmark.
//!
//! Inline formatting (bold / italic / inline code / links) is flattened
//! to plain text — getting per-span styling right inside GPUI requires
//! the same `StyledText::with_highlights` plumbing we use for diff lines
//! and isn't worth the complexity for a v1. Block-level structure
//! (headings, paragraphs, lists, code, quotes, rule) is preserved.
//!
//! Mermaid fenced code blocks (lang == "mermaid") render as a monospace
//! block with a note pointing the user at the React UI — GPUI has no
//! WebView, so we can't actually run mermaid.js.

use gpui::{div, prelude::*, px, IntoElement, ParentElement, SharedString, Styled};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::ui::theme::{Theme, MONO_FONT, UI_FONT};

#[derive(Debug, Clone)]
enum Block {
    Heading { level: u8, text: String },
    Paragraph(String),
    List { items: Vec<String>, ordered: bool },
    Code { lang: Option<String>, content: String },
    Mermaid(String),
    BlockQuote(String),
    Rule,
}

pub fn render_markdown(source: &str, font_size: f32) -> impl IntoElement {
    let blocks = parse(source);
    let body = div()
        .id("md-scroll")
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_y_scroll()
        .px_6()
        .py_4()
        .font_family(UI_FONT)
        .text_size(px(font_size))
        .text_color(Theme::TEXT)
        .flex()
        .flex_col()
        .gap_2();

    blocks.into_iter().fold(body, |acc, b| acc.child(render_block(b, font_size)))
}

fn render_block(block: Block, font_size: f32) -> gpui::AnyElement {
    match block {
        Block::Heading { level, text } => {
            let scale = match level {
                1 => 2.0,
                2 => 1.6,
                3 => 1.4,
                4 => 1.2,
                5 => 1.1,
                _ => 1.0,
            };
            div()
                .mt_3()
                .text_size(px(font_size * scale))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(Theme::TEXT)
                .child(SharedString::from(text))
                .into_any_element()
        }
        Block::Paragraph(text) => div()
            .text_color(Theme::TEXT)
            .child(SharedString::from(text))
            .into_any_element(),
        Block::List { items, ordered } => {
            let mut col = div().flex().flex_col().gap_1();
            for (i, item) in items.into_iter().enumerate() {
                let prefix = if ordered {
                    format!("{}. ", i + 1)
                } else {
                    "• ".to_string()
                };
                col = col.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .w(px(24.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child(SharedString::from(prefix)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(Theme::TEXT)
                                .child(SharedString::from(item)),
                        ),
                );
            }
            col.into_any_element()
        }
        Block::Code { lang, content } => div()
            .px_3()
            .py_2()
            .bg(Theme::BG_ELEVATED)
            .border_1()
            .border_color(Theme::BORDER)
            .rounded_sm()
            .font_family(MONO_FONT)
            .text_size(px(font_size * 0.95))
            .text_color(Theme::TEXT)
            .child(
                div()
                    .text_color(Theme::TEXT_MUTED)
                    .text_size(px(font_size * 0.8))
                    .child(SharedString::from(
                        lang.unwrap_or_default(),
                    )),
            )
            .child(SharedString::from(content))
            .into_any_element(),
        Block::Mermaid(content) => div()
            .px_3()
            .py_2()
            .bg(Theme::BG_ELEVATED)
            .border_1()
            .border_color(Theme::TEXT_LINK)
            .rounded_sm()
            .font_family(MONO_FONT)
            .text_size(px(font_size * 0.95))
            .text_color(Theme::TEXT)
            .child(
                div()
                    .text_color(Theme::TEXT_LINK)
                    .text_size(px(font_size * 0.8))
                    .child(SharedString::from(
                        "mermaid (preview requires the React UI)",
                    )),
            )
            .child(SharedString::from(content))
            .into_any_element(),
        Block::BlockQuote(text) => div()
            .pl_3()
            .border_l_1()
            .border_color(Theme::BORDER)
            .text_color(Theme::TEXT_MUTED)
            .child(SharedString::from(text))
            .into_any_element(),
        Block::Rule => div()
            .my_2()
            .h(px(1.0))
            .w_full()
            .bg(Theme::BORDER)
            .into_any_element(),
    }
}

fn parse(source: &str) -> Vec<Block> {
    let parser = Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let mut blocks: Vec<Block> = Vec::new();
    let mut current_text = String::new();
    let mut in_code: Option<String> = None;
    let mut code_lang: Option<String> = None;
    let mut list_stack: Vec<(bool /* ordered */, Vec<String> /* items */)> = Vec::new();
    let mut item_text = String::new();
    let mut in_quote = false;
    let mut heading_level: Option<u8> = None;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(heading_level_to_u8(level));
                current_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(lvl) = heading_level.take() {
                    blocks.push(Block::Heading {
                        level: lvl,
                        text: std::mem::take(&mut current_text),
                    });
                }
            }
            Event::Start(Tag::Paragraph) => current_text.clear(),
            Event::End(TagEnd::Paragraph) => {
                let text = std::mem::take(&mut current_text);
                if !text.is_empty() {
                    if !list_stack.is_empty() {
                        item_text.push_str(&text);
                    } else if in_quote {
                        blocks.push(Block::BlockQuote(text));
                    } else {
                        blocks.push(Block::Paragraph(text));
                    }
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let s = lang.to_string();
                        if s.is_empty() { None } else { Some(s) }
                    }
                    CodeBlockKind::Indented => None,
                };
                in_code = Some(String::new());
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(content) = in_code.take() {
                    let lang = code_lang.take();
                    if lang.as_deref() == Some("mermaid") {
                        blocks.push(Block::Mermaid(content));
                    } else {
                        blocks.push(Block::Code { lang, content });
                    }
                }
            }
            Event::Start(Tag::List(start)) => {
                list_stack.push((start.is_some(), Vec::new()));
            }
            Event::End(TagEnd::List(_)) => {
                if let Some((ordered, items)) = list_stack.pop() {
                    blocks.push(Block::List { items, ordered });
                }
            }
            Event::Start(Tag::Item) => {
                item_text.clear();
                current_text.clear();
            }
            Event::End(TagEnd::Item) => {
                if let Some((_, items)) = list_stack.last_mut() {
                    let mut text = std::mem::take(&mut item_text);
                    text.push_str(&std::mem::take(&mut current_text));
                    items.push(text);
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                in_quote = true;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_quote = false;
            }
            Event::Rule => blocks.push(Block::Rule),
            Event::Text(t) => {
                if let Some(buf) = in_code.as_mut() {
                    buf.push_str(&t);
                } else {
                    current_text.push_str(&t);
                }
            }
            Event::Code(t) => {
                current_text.push('`');
                current_text.push_str(&t);
                current_text.push('`');
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(buf) = in_code.as_mut() {
                    buf.push('\n');
                } else {
                    current_text.push(' ');
                }
            }
            _ => {}
        }
    }

    blocks
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
