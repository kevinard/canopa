//! Document layout is implemented in Rust; parsers/converters run in a private
//! namespace with only the approved input and bounded anonymous output streams.
mod pdf;
mod plan;
pub(crate) mod runner;
use super::{Format, Layout, PageChoices, PageSetup};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Layout,
    Copies,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub layout: Layout,
    pub page_setup: PageSetup,
    pub format: Format,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub duplex: bool,
}
impl Request {
    pub fn copies(copies: u32, collate: bool, duplex: bool, format: Format) -> Self {
        Self {
            layout: Layout {
                copies,
                collate,
                ..Default::default()
            },
            page_setup: PageSetup::default(),
            format,
            mode: Mode::Copies,
            duplex,
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        self.layout.validate(PageChoices::default())?;
        self.page_setup.validate()?;
        if self.mode == Mode::Copies {
            anyhow::ensure!(
                self.layout
                    == (Layout {
                        copies: self.layout.copies,
                        collate: self.layout.collate,
                        ..Default::default()
                    })
                    && self.page_setup == PageSetup::default(),
                "Copy-only rendering cannot apply another document layout"
            );
        }
        Ok(())
    }
}

/// Runs only in the converter namespace. Its stdin is a bounded sealed PDF;
/// stdout is a distinct fresh memfd. No D-Bus or application runtime is started.
pub fn run_layout_worker() -> anyhow::Result<()> {
    let input = std::env::var("CANOPA_PRINT_LAYOUT")?;
    anyhow::ensure!(
        input.len() <= super::MAX_REQUEST_BYTES as usize,
        "Print layout exceeds limits"
    );
    let request: Request = serde_json::from_str(&input)?;
    request.validate()?;
    pdf::render(&request)
}
