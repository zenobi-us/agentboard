pub mod jira;
pub mod qmd;

use anyhow::Result;

use crate::model::{Item, SourceConfig, SourceKind};
use jira::JiraSource;
use qmd::QmdSource;

#[allow(async_fn_in_trait)]
pub trait SourceAdapter {
    async fn collect(&self, source: &SourceConfig) -> Result<Vec<Item>>;
    // TODO: validate/auth/pagination hooks when network sources land.
}

pub async fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    match &source.source {
        SourceKind::Qmd { .. } => QmdSource.collect(source).await,
        SourceKind::Jira { .. } => JiraSource.collect(source).await,
    }
}
