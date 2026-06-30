use crate::models::{Item, ItemKind};
use chrono::Utc;
use uuid::Uuid;

#[cfg(debug_assertions)]
pub fn mock_items() -> Vec<Item> {
    vec![
        Item {
            id: Uuid::new_v4(),
            name: "Start docker compose".into(),
            kind: ItemKind::Command,
            content: "docker compose up -d".into(),
            language: None,
            tags: vec!["docker".into(), "devops".into()],
            created_at: Utc::now(),
            modified_at: Utc::now(),
        },
        Item {
            id: Uuid::new_v4(),
            name: "Axum handler".into(),
            kind: ItemKind::Snippet,
            content: r#"async fn handler(State(db): State<DbPool>) -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}"#
            .into(),
            language: Some("rust".into()),
            tags: vec!["rust".into(), "axum".into()],
            created_at: Utc::now(),
            modified_at: Utc::now(),
        },
        Item {
            id: Uuid::new_v4(),
            name: "Architecture notes".into(),
            kind: ItemKind::Note,
            content: "# Architecture\n\nThe app uses GPUI for rendering...".into(),
            language: None,
            tags: vec!["notes".into()],
            created_at: Utc::now(),
            modified_at: Utc::now(),
        },
    ]
}
