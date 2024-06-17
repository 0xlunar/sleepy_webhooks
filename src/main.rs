mod routes;
mod db;
mod pool;

use clap::Parser;
use std::sync::Arc;
use actix_web::{App, HttpServer, web};
use serde::Deserialize;
use crate::db::{DBConnection, WebhookDB};
use crate::pool::PoolItem;
use crate::routes::{get_webhook_details, send_delayed_webhook, update_delayed_webhook_settings, delete_delayed_webhook, create_delayed_webhook};

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = Arguments::parse();

    let port = args.port
        .unwrap_or_else(||
            std::env::var("port")
                .map_or(8080, |port|
                    port.parse().unwrap()
                )
        );

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
    let sender= web::Data::new(tx);

    let pool_handle = pool.start();

    HttpServer::new(move ||
        App::new()
            .app_data(web::Data::clone(&db))
            .app_data(web::Data::clone(&sender))
            .service(get_webhook_details)
            .service(send_delayed_webhook)
            .service(update_delayed_webhook_settings)
            .service(delete_delayed_webhook)
            .service(create_delayed_webhook)
        )
        .bind(("0.0.0.0", port))?
        .run().await?;

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
