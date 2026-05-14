-- Per-webspace secret for the webhook callback URL.
ALTER TABLE webspaces ADD COLUMN changedetection_secret TEXT;

-- Incoming notification storage.
CREATE TABLE changedetection_notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webspace_id UUID NOT NULL REFERENCES webspaces(id) ON DELETE CASCADE,
    title TEXT,
    body TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_cdn_webspace ON changedetection_notifications(webspace_id, created_at DESC);
