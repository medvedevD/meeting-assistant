use std::sync::Arc;
use crate::app_core::AppCore;
use crate::types::{AppError, FfiResult};

#[uniffi::export(async_runtime = "tokio")]
impl AppCore {
    pub async fn templates_list(self: Arc<Self>) -> FfiResult<Vec<String>> {
        self.templates.list_names().await.map_err(AppError::general)
    }
}
