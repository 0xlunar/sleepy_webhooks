# sleepy_webhooks
Add a delay before sending webhooks, with the option to send instantly to specific webhooks.
Allows mirroring of webhooks for when you need to send to multiple places.

### Arguments
```commandline
cargo run -- -d "postgresql://user:pass@ip:port/database" -p 80
```
```commandline
Usage: sleepy_webhooks.exe [OPTIONS]

Options:
  -d, --database-uri <DATABASE_URI>
  -p, --port <PORT>
  -h, --help                         Print help

```

### Endpoints
- `GET /webhooks` Returns all defined webhooks
- `GET /webhook/{id}` Returns the config for a given delayed webhook
- `POST /webhook/{id}` Submit Webhook to be delayed 
  - Payload sent is mimicked to all webhooks in the given config
- `PATCH /webhook/{id}` Update Delayed Webhooks config
  - JSON Payload (All fields are optional)
    - `delay: Option<i64>`
    - `name: String`
    - `remove_delayed: Option<Vec<String>>`
    - `append_delayed: Option<Vec<String>>`
    - `remove_instant: Option<Vec<String>>`
    - `append_instant: Option<Vec<String>>`
- `DELETE /webhook/{id}` Delete a given delayed webhook config
- `POST /create` Create a new Delayed webhook config
  - JSON Payload (Instant webhooks are optional)
    - `delay: i64`
    - `name: String`
    - `delayed_webhooks: Vec<String>`
    - `instant_webhooks: Vec<String>` Defaults to empty vec