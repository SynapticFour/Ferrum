use std::sync::Arc;

pub struct AppState {
    pub pool: sqlx::PgPool,
    pub activity: Arc<crate::activity::ActivityLogger>,
}
