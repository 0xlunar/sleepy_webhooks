use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use actix_web::web::{Bytes, BytesMut};
use anyhow::format_err;
use chrono::Local;
use log::error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use rayon::prelude::*;
use reqwest::ClientBuilder;
use tokio::sync::mpsc::UnboundedReceiver;
use crate::db::{DBConnection, WebhookDB, WebhookDBItem};

pub struct Pool {
    pool: VecDeque<PoolItem>,
    receiver: UnboundedReceiver<PoolItem>,
    db: Arc<DBConnection>
}

pub struct PoolItem {
    id: String,
    received_at: chrono::DateTime<Local>,
    instant_sent: bool,
    delay_sent: bool,
    data: Bytes,
}

impl PoolItem {
    pub fn new(id: String, data: BytesMut) -> Self {
        Self {
            id,
            received_at: Local::now(),
            instant_sent: false,
            delay_sent: false,
            data: data.freeze()
        }
    }
}

impl Pool {
    pub fn new(receiver: UnboundedReceiver<PoolItem>, db: Arc<DBConnection>) -> Self {
        Self {
            pool: VecDeque::new(),
            receiver,
            db
        }
    }

    pub fn start(self) -> JoinHandle<()> {
        tokio::task::spawn(async move {
            let db = WebhookDB::new(self.db);
            let webhook_db = Arc::new(db);
            let mut pool = self.pool;
            let mut receiver = self.receiver;
            loop {
                let mut interval = tokio::time::interval(Duration::from_millis(100));
                while let Some(val) = tokio::select! {
                    Some(val) = receiver.recv() => {
                        Some(val)
                    }
                    _ = interval.tick() => None
                } {
                    pool.push_back(val);
                }

                let db_entries = pool.iter_mut().map(|item| {
                    Pool::process_pool_item(item, Arc::clone(&webhook_db))
                }).collect::<Vec<_>>();

                futures_util::future::join_all(db_entries).await;

                pool = pool.into_par_iter().filter(|item| !item.delay_sent).collect();
            }
        })
    }

    async fn process_pool_item(item: &mut PoolItem, db: Arc<WebhookDB>) -> anyhow::Result<()> {
        let client = ClientBuilder::new().build()?;

        let db_item = db.get(&item.id).await?;

        let now = Local::now();
        let mut delayed_time = now.clone();
        delayed_time = delayed_time
            .checked_sub_signed(chrono::Duration::seconds(db_item.delay_seconds))
            .unwrap();

        let mut requests = Vec::new();

        if !item.instant_sent {
            let data = String::from_utf8_lossy(&item.data).to_string();
            let mut instant_hooks = db_item.instant_webhooks
                .par_iter()
                .map(|wh| client.post(wh).header("Content-Type", "application/json").body(data.clone()).send())
                .collect::<Vec<_>>();

            item.instant_sent = true;
            requests.append(&mut instant_hooks);
        }

        // if our sent time is smaller than (now - delay_seconds) then it has passed the delay and we can send.
        if item.received_at.lt(&delayed_time) && !item.delay_sent {
            let data = String::from_utf8_lossy(&item.data).to_string();
            let mut delay_hooks = db_item.delay_webhooks
                .par_iter()
                .map(|wh| client.post(wh).header("Content-Type", "application/json").body(data.clone()).send())
                .collect::<Vec<_>>();

            item.delay_sent = true;
            requests.append(&mut delay_hooks);
        }

        futures_util::future::join_all(requests)
            .await
            .into_par_iter()
            .filter_map(|res| {
                match res {
                    Ok(resp) => {
                        if resp.status().is_server_error() || resp.status().is_client_error() {
                            Some(format_err!("Status: {}, URL: {}", resp.status(), resp.url()))
                        } else {
                            None
                        }
                    },
                    Err(e) => Some(e.into())
                }
            })
            .for_each(|err| error!("{}", err));

        Ok(())
    }
}

