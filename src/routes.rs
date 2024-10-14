use crate::db::{DBConnection, WebhookDB};
use crate::pool::PoolItem;
use actix_web::{delete, get, patch, post, web, HttpResponse};
use futures_util::StreamExt as _;
use log::error;
use rayon::prelude::*;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;

#[get("/webhooks")]
async fn get_webhooks(db: web::Data<DBConnection>) -> actix_web::Result<HttpResponse<String>> {
    let webhook_db = WebhookDB::new(db.into_inner());

    match webhook_db.fetch_all().await {
        Ok(t) => {
            let output = serde_json::to_string(&t).unwrap();
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .message_body(output)
                .unwrap())
        }
        Err(e) => Ok(HttpResponse::NotFound()
            .message_body(format!("ERROR|{}", e))
            .unwrap()),
    }
}

#[get("/webhook/{id}")]
async fn get_webhook_details(
    id: web::Path<String>,
    db: web::Data<DBConnection>,
) -> actix_web::Result<HttpResponse<String>> {
    let id = id.into_inner();

    let webhook_db = WebhookDB::new(db.into_inner());
    match webhook_db.get(&id).await {
        Ok(t) => {
            let output = serde_json::to_string(&t).unwrap();
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .message_body(output)
                .unwrap())
        }
        Err(e) => Ok(HttpResponse::NotFound()
            .message_body(format!("ERROR|{}\n{}", id, e))
            .unwrap()),
    }
}

#[post("/webhook/{id}")]
async fn send_delayed_webhook(
    id: web::Path<String>,
    mut payload: web::Payload,
    db: web::Data<DBConnection>,
    sender: web::Data<UnboundedSender<PoolItem>>,
) -> actix_web::Result<HttpResponse<String>> {
    let id = id.into_inner();
    let webhook_db = WebhookDB::new(db.into_inner());

    match webhook_db.get(&id).await {
        Ok(_) => {
            let mut bytes = web::BytesMut::new();
            while let Some(item) = payload.next().await {
                bytes.extend_from_slice(&item?)
            }

            let pool_item = PoolItem::new(id.clone(), bytes);
            sender.send(pool_item).unwrap();

            Ok(HttpResponse::Ok()
                .message_body(format!("ADDED|{}", id))
                .unwrap())
        }
        Err(_) => Ok(HttpResponse::NotFound()
            .message_body(format!("ERROR|Invalid ID: {}", id))
            .unwrap()),
    }
}

#[derive(Deserialize)]
struct PatchWebhookSettings {
    delay: Option<i64>,
    name: Option<String>,
    remove_delayed: Option<Vec<String>>,
    append_delayed: Option<Vec<String>>,
    remove_instant: Option<Vec<String>>,
    append_instant: Option<Vec<String>>,
}

#[patch("/webhook/{id}")]
async fn update_delayed_webhook_settings(
    id: web::Path<String>,
    payload: web::Json<PatchWebhookSettings>,
    db: web::Data<DBConnection>,
) -> actix_web::Result<HttpResponse<String>> {
    let id = id.into_inner();
    let webhook_db = WebhookDB::new(db.into_inner());
    let mut all_errs = Vec::new();

    match &payload.delay {
        Some(d) => {
            match webhook_db.update_delay_seconds(id.to_string(), *d).await {
                Ok(_) => (),
                Err(e) => all_errs.push(e),
            };
        }
        None => (),
    }

    match &payload.name {
        Some(name) => match webhook_db.update_name(&id, name).await {
            Ok(_) => (),
            Err(e) => all_errs.push(e),
        },
        None => (),
    }

    match &payload.remove_delayed {
        Some(to_remove) => {
            let reqs = to_remove
                .iter()
                .map(|wh| webhook_db.remove_delayed_webhook(id.clone(), wh.clone()))
                .collect::<Vec<_>>();

            let mut errs = futures_util::future::join_all(reqs)
                .await
                .into_par_iter()
                .filter_map(|res| match res {
                    Ok(_) => None,
                    Err(e) => Some(e),
                })
                .collect::<Vec<_>>();
            all_errs.append(&mut errs);
        }
        None => (),
    }

    match &payload.append_delayed {
        Some(to_append) => {
            let reqs = to_append
                .iter()
                .map(|wh| webhook_db.add_delayed_webhook(id.clone(), wh.clone()))
                .collect::<Vec<_>>();

            let mut errs = futures_util::future::join_all(reqs)
                .await
                .into_par_iter()
                .filter_map(|res| match res {
                    Ok(_) => None,
                    Err(e) => Some(e),
                })
                .collect::<Vec<_>>();
            all_errs.append(&mut errs);
        }
        None => (),
    }

    match &payload.remove_instant {
        Some(to_remove) => {
            let reqs = to_remove
                .iter()
                .map(|wh| webhook_db.remove_instant_webhook(id.clone(), wh.clone()))
                .collect::<Vec<_>>();

            let mut errs = futures_util::future::join_all(reqs)
                .await
                .into_par_iter()
                .filter_map(|res| match res {
                    Ok(_) => None,
                    Err(e) => Some(e),
                })
                .collect::<Vec<_>>();
            all_errs.append(&mut errs);
        }
        None => (),
    }

    match &payload.append_instant {
        Some(to_append) => {
            let reqs = to_append
                .iter()
                .map(|wh| webhook_db.add_instant_webhook(id.clone(), wh.clone()))
                .collect::<Vec<_>>();

            let mut errs = futures_util::future::join_all(reqs)
                .await
                .into_par_iter()
                .filter_map(|res| match res {
                    Ok(_) => None,
                    Err(e) => Some(e),
                })
                .collect::<Vec<_>>();
            all_errs.append(&mut errs);
        }
        None => (),
    }

    if all_errs.len() > 0 {
        let mut output = String::new();
        for err in all_errs {
            error!("{}", err);
            let mut err = err.to_string();
            err.push_str("\n");
            output.push_str(&err);
        }

        return Ok(HttpResponse::InternalServerError()
            .message_body(format!("ERROR|{}\n{}", id, output))
            .unwrap());
    }

    Ok(HttpResponse::Ok()
        .message_body(format!("PATCHED|{}", id))
        .unwrap())
}

#[delete("/webhook/{id}")]
async fn delete_delayed_webhook(
    id: web::Path<String>,
    db: web::Data<DBConnection>,
) -> actix_web::Result<HttpResponse<String>> {
    let id = id.into_inner();

    let webhook_db = WebhookDB::new(db.into_inner());
    match webhook_db.delete(id.clone()).await {
        Ok(_) => Ok(HttpResponse::Ok()
            .message_body(format!("DELETED|{}", id))
            .unwrap()),
        Err(e) => Ok(HttpResponse::InternalServerError()
            .message_body(format!("ERROR|{}\n{}", id, e))
            .unwrap()),
    }
}

#[derive(Deserialize)]
struct CreateDelayedWebhookPayload {
    delay: i64,
    name: String,
    delayed_webhooks: Vec<String>,
    #[serde(default)]
    instant_webhooks: Vec<String>,
}

#[post("/create")]
async fn create_delayed_webhook(
    data: web::Json<CreateDelayedWebhookPayload>,
    db: web::Data<DBConnection>,
) -> actix_web::Result<HttpResponse<String>> {
    let webhook_db = WebhookDB::new(db.into_inner());

    let id = match webhook_db
        .create(
            data.delay,
            &data.name,
            &data.delayed_webhooks,
            &data.instant_webhooks,
        )
        .await
    {
        Ok(t) => t,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .message_body(format!("ERROR|{}", e))
                .unwrap())
        }
    };

    Ok(HttpResponse::Ok()
        .message_body(format!("CREATED|{}", id))
        .unwrap())
}
