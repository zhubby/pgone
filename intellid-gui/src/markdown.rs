use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn render_markdown(ui: &mut egui::Ui, text: &str) {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(text, opts);

    let mut in_code_block = false;
    let mut code_buf = String::new();
    let mut current_link: Option<String> = None;
    let mut heading_level: Option<HeadingLevel> = None;
    let mut heading_buf = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(_kind)) => {
                in_code_block = true;
                code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if in_code_block {
                    ui.code(&code_buf);
                    code_buf.clear();
                    in_code_block = false;
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                current_link = Some(dest_url.to_string());
            }
            Event::End(TagEnd::Link) => {
                current_link = None;
            }
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(level);
                heading_buf.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if heading_level.take().is_some() {
                    ui.heading(heading_buf.clone());
                    heading_buf.clear();
                }
            }
            Event::Code(inline) => {
                if in_code_block {
                    code_buf.push_str(&inline);
                } else {
                    ui.code(inline.as_ref());
                }
            }
            Event::Text(t) => {
                if in_code_block {
                    code_buf.push_str(&t);
                } else if heading_level.is_some() {
                    heading_buf.push_str(&t);
                } else if let Some(url) = &current_link {
                    if ui.link(t.as_ref()).clicked() {
                        let _ = webbrowser::open(url);
                    }
                } else {
                    ui.label(t.as_ref());
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                ui.add_space(4.0);
            }
            _ => {}
        }
    }
    if in_code_block && !code_buf.is_empty() {
        ui.code(&code_buf);
    }
    if heading_level.is_some() && !heading_buf.is_empty() {
        ui.heading(heading_buf);
    }
}
