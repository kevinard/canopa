use super::FormatValue;
use super::style::{ResolvedStyle, SegmentStyles, Style};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormatConfig {
    pub collapsed: Option<FormatTemplate>,
    pub expanded: Option<FormatTemplate>,
    pub horizontal: Option<FormatTemplate>,
}

impl FormatConfig {
    pub fn template_for(&self, mode: FormatMode) -> Option<&FormatTemplate> {
        match mode {
            FormatMode::Collapsed => self.collapsed.as_ref(),
            FormatMode::Expanded => self.expanded.as_ref().or(self.collapsed.as_ref()),
            FormatMode::Horizontal => self
                .horizontal
                .as_ref()
                .or(self.expanded.as_ref())
                .or(self.collapsed.as_ref()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum TooltipSetting {
    #[default]
    Default,
    Disabled,
    Custom(FormatTemplate),
}

impl TooltipSetting {
    pub fn resolve(&self, default_template: Option<FormatTemplate>) -> Option<FormatTemplate> {
        match self {
            TooltipSetting::Default => default_template,
            TooltipSetting::Disabled => None,
            TooltipSetting::Custom(t) => Some(t.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    Collapsed,
    Expanded,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTemplate {
    segments: Vec<StyledSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledSegment {
    template: TextTemplate,
    styles: SegmentStyles,
}

impl StyledSegment {
    pub fn new(template: TextTemplate) -> Self {
        StyledSegment {
            template,
            styles: SegmentStyles::None,
        }
    }

    pub fn set_styles(&mut self, styles: SegmentStyles) {
        self.styles = styles;
    }
}

impl FormatTemplate {
    #[cfg(test)]
    pub fn parse(raw_segments: &[&str], allowed: &[&str], path: &str) -> anyhow::Result<Self> {
        if raw_segments.is_empty() {
            anyhow::bail!("{path}: expected at least one string value.");
        }
        let mut segments = Vec::with_capacity(raw_segments.len());
        for raw in raw_segments {
            segments.push(StyledSegment::new(TextTemplate::parse(raw, allowed, path)?));
        }
        Ok(FormatTemplate { segments })
    }

    pub fn from_styled(segments: Vec<StyledSegment>) -> Self {
        FormatTemplate { segments }
    }

    #[cfg(test)]
    pub fn set_segment_styles(&mut self, index: usize, styles: SegmentStyles) {
        if let Some(s) = self.segments.get_mut(index) {
            s.set_styles(styles);
        }
    }

    pub fn render_segments(&self, values: &[(&str, String)]) -> Vec<String> {
        self.segments
            .iter()
            .map(|s| s.template.render_pairs(values))
            .collect()
    }

    pub fn render_joined(&self, values: &[(&str, String)]) -> String {
        self.render_segments(values).join(" ")
    }

    pub fn render_styled_segments_with_default(
        &self,
        values: &[(&str, String)],
        state: Option<&str>,
        default_style: &Style,
    ) -> Vec<(String, ResolvedStyle)> {
        self.segments
            .iter()
            .map(|s| {
                (
                    s.template.render_pairs(values),
                    s.styles
                        .style_for(state)
                        .merge_over(*default_style)
                        .resolve(),
                )
            })
            .collect()
    }

    pub fn render_styled_cow_segments_with_default(
        &self,
        values: &[FormatValue<'_>],
        state: Option<&str>,
        default_style: &Style,
    ) -> Vec<(String, ResolvedStyle)> {
        self.segments
            .iter()
            .map(|s| {
                (
                    s.template.render_cow_pairs(values),
                    s.styles
                        .style_for(state)
                        .merge_over(*default_style)
                        .resolve(),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextTemplate {
    parts: Vec<TemplatePart>,
}

#[derive(Debug, Clone, PartialEq)]
enum TemplatePart {
    Literal(String),
    Placeholder(String),
}

impl TextTemplate {
    pub fn parse(raw: &str, allowed: &[&str], path: &str) -> anyhow::Result<Self> {
        if raw.is_empty() {
            anyhow::bail!("{path}: value must not be empty.");
        }

        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut chars = raw.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        literal.push('{');
                        continue;
                    }
                    if !literal.is_empty() {
                        parts.push(TemplatePart::Literal(std::mem::take(&mut literal)));
                    }

                    let mut name = String::new();
                    let mut closed = false;
                    for inner in chars.by_ref() {
                        if inner == '}' {
                            closed = true;
                            break;
                        }
                        name.push(inner);
                    }

                    if !closed {
                        anyhow::bail!("{path}: unmatched '{{' in format template.");
                    }
                    if name.is_empty() {
                        anyhow::bail!("{path}: empty placeholder is not allowed.");
                    }
                    if !name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    {
                        anyhow::bail!("{path}: invalid placeholder {name:?}.");
                    }
                    if !allowed.contains(&name.as_str()) {
                        anyhow::bail!("{path}: unknown placeholder {name:?}.");
                    }
                    parts.push(TemplatePart::Placeholder(name));
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        literal.push('}');
                    } else {
                        anyhow::bail!("{path}: unmatched '}}' in format template.");
                    }
                }
                _ => literal.push(ch),
            }
        }

        if !literal.is_empty() {
            parts.push(TemplatePart::Literal(literal));
        }

        Ok(TextTemplate { parts })
    }

    pub fn render_pairs(&self, values: &[(&str, String)]) -> String {
        let mut rendered = String::new();
        for part in &self.parts {
            match part {
                TemplatePart::Literal(value) => rendered.push_str(value),
                TemplatePart::Placeholder(name) => {
                    if let Some((_, value)) = values.iter().find(|(key, _)| key == &name.as_str()) {
                        rendered.push_str(value);
                    }
                }
            }
        }
        rendered
    }

    pub fn render_cow_pairs(&self, values: &[FormatValue<'_>]) -> String {
        let mut rendered = String::new();
        for part in &self.parts {
            match part {
                TemplatePart::Literal(value) => rendered.push_str(value),
                TemplatePart::Placeholder(name) => {
                    if let Some((_, value)) = values.iter().find(|(key, _)| key == &name.as_str()) {
                        rendered.push_str(value);
                    }
                }
            }
        }
        rendered
    }
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod template_tests;
