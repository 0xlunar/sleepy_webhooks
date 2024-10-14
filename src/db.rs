use log::info;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Uuid;
use sqlx::{Error, Executor, PgPool, Row};
use std::sync::Arc;

type PSQLResult = Result<(), Error>;

pub struct DBConnection {
    db: PgPool,
}
impl DBConnection {
    pub async fn new(connection_uri: &str) -> Self {
        let db = PgPoolOptions::new()
            // .max_connections(100)
            .connect(connection_uri)
            .await
            .unwrap();

        Self { db }
    }
}

pub struct WebhookDB {
    db: Arc<DBConnection>,
}
#[derive(sqlx::FromRow, Serialize, Debug)]
pub struct WebhookDBItem {
    pub id: String,
    pub name: String,
    pub delay_seconds: i64,
    pub delay_webhooks: Vec<String>,
    pub instant_webhooks: Vec<String>,
}

impl WebhookDB {
    pub fn new(db: Arc<DBConnection>) -> Self {
        Self { db }
    }

    pub async fn initialise(&self) -> anyhow::Result<()> {
        let mut tx = self.db.db.begin().await?;
        let sql = include_str!("webhook_db.sql");
        // Doesn't return anything useful on success or error so can ignore, if it fails the app just won't work
        tx.execute(sql).await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> anyhow::Result<WebhookDBItem> {
        let item = sqlx::query_as("SELECT * FROM webhooks WHERE id = $1")
            .bind(&id)
            .fetch_one(&self.db.db)
            .await?;

        Ok(item)
    }

    pub async fn create(
        &self,
        delay_seconds: i64,
        name: &str,
        delayed_webhooks: &[String],
        instant_webhooks: &[String],
    ) -> Result<String, Error> {
        let item = sqlx::query(
            "INSERT INTO webhooks(delay_seconds, name, delay_webhooks, instant_webhooks) VALUES ($1, $2, $3, $4) RETURNING id"
        )
            .bind(delay_seconds)
            .bind(name)
            .bind(delayed_webhooks)
            .bind(instant_webhooks)
            .fetch_one(&self.db.db)
            .await?;

        let item: String = item.get(0);

        Ok(item)
    }

    pub async fn fetch_all(&self) -> anyhow::Result<Vec<WebhookDBItem>> {
        let results = sqlx::query_as("SELECT * FROM webhooks")
            .fetch_all(&self.db.db)
            .await?;
        Ok(results)
    }

    pub async fn update_name(&self, id: &str, name: &str) -> PSQLResult {
        sqlx::query("UPDATE webhooks SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&self.db.db)
            .await?;

        Ok(())
    }

    pub async fn update_delay_seconds(&self, id: String, delay_seconds: i64) -> PSQLResult {
        sqlx::query("UPDATE webhooks SET delay_seconds = $1 WHERE id = $2")
            .bind(delay_seconds)
            .bind(id)
            .execute(&self.db.db)
            .await?;

        Ok(())
    }

    pub async fn add_delayed_webhook(&self, id: String, delay_webhook: String) -> PSQLResult {
        sqlx::query(
            "UPDATE webhooks SET delay_webhooks = array_append(delay_webhooks, $1) WHERE id = $2",
        )
        .bind(delay_webhook)
        .bind(id)
        .execute(&self.db.db)
        .await?;
        Ok(())
    }

    pub async fn remove_delayed_webhook(&self, id: String, delay_webhook: String) -> PSQLResult {
        sqlx::query(
            "UPDATE webhooks SET delay_webhooks = array_remove(delay_webhooks, $1) WHERE id = $2",
        )
        .bind(delay_webhook)
        .bind(id)
        .execute(&self.db.db)
        .await?;
        Ok(())
    }

    pub async fn add_instant_webhook(&self, id: String, instant_webhook: String) -> PSQLResult {
        sqlx::query("UPDATE webhooks SET instant_webhooks = array_append(instant_webhooks, $1) WHERE id = $2")
            .bind(instant_webhook)
            .bind(id)
            .execute(&self.db.db)
            .await?;
        Ok(())
    }

    pub async fn remove_instant_webhook(&self, id: String, instant_webhook: String) -> PSQLResult {
        sqlx::query("UPDATE webhooks SET instant_webhooks = array_remove(instant_webhooks, $1) WHERE id = $2")
            .bind(instant_webhook)
            .bind(id)
            .execute(&self.db.db)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: String) -> PSQLResult {
        sqlx::query("DELETE FROM webhooks WHERE id = $1")
            .bind(id)
            .execute(&self.db.db)
            .await?;
        Ok(())
    }
}
