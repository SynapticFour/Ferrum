use std::sync::Arc;

pub struct AppState {
    pub pool: sqlx::PgPool,
    pub activity: Arc<crate::activity::ActivityLogger>,
    pub email_sender: Option<Arc<dyn crate::email::EmailSender>>,
    pub invite_base_url: Option<String>,
}
