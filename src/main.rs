mod db;
mod pool;
mod routes;

use crate::db::{DBConnection, WebhookDB};
use crate::pool::PoolItem;
use crate::routes::{create_delayed_webhook, delete_delayed_webhook, get_webhook_details, get_webhooks, send_delayed_webhook, update_delayed_webhook_settings};
use actix_web::{web, App, HttpServer};
use clap::Parser;
use serde::Deserialize;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = Arguments::parse();

    let port = args
        .port
        .unwrap_or_else(|| std::env::var("port").map_or(8080, |port| port.parse().unwrap()));

    let db = match args.database_uri {
        Some(t) => web::Data::new(DBConnection::new(&t).await),
        None => {
            let db_uri = std::env::var("db").unwrap();
            web::Data::new(DBConnection::new(&db_uri).await)
        }
    };

    let webhook_db = WebhookDB::new(Arc::clone(&db));
    webhook_db.initialise().await?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PoolItem>();
    let db_clone = web::Data::clone(&db);
    let pool = pool::Pool::new(rx, db_clone.into_inner());
    let sender = web::Data::new(tx);

    let pool_handle = pool.start();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::clone(&db))
            .app_data(web::Data::clone(&sender))
            .service(get_webhook_details)
            .service(send_delayed_webhook)
            .service(update_delayed_webhook_settings)
            .service(delete_delayed_webhook)
            .service(create_delayed_webhook)
            .service(get_webhooks)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await?;

    pool_handle.await?;

    Ok(())
}

#[derive(Parser, Debug)]
struct Arguments {
    #[arg(short = 'd', long)]
    database_uri: Option<String>,
    #[arg(short, long)]
    port: Option<u16>,
}
