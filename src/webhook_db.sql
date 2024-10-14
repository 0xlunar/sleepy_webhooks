CREATE TABLE IF NOT EXISTS webhooks
(
    id TEXT PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    name TEXT NOT NULL DEFAULT 'webhook',
    delay_seconds BIGINT NOT NULL DEFAULT 0,
    delay_webhooks TEXT[] NOT NULL DEFAULT array[]::TEXT[],
    instant_webhooks TEXT[] NOT NULL DEFAULT array[]::TEXT[]
);