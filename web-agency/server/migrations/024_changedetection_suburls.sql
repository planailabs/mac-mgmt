-- Sub-URL tracking for ChangeDetection.io
-- Each sub-URL gets its own tag, webhook secret, and tag settings.
-- The webspace-level tag ({group}:{webspace}) is kept for human filtering;
-- sub-URL tags ({group}:{webspace}:{path}) carry notification webhooks and settings.

CREATE TABLE changedetection_suburls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webspace_id UUID NOT NULL REFERENCES webspaces(id) ON DELETE CASCADE,
    path TEXT NOT NULL DEFAULT '/',
    tag_id UUID,
    secret TEXT NOT NULL,
    tag_settings JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (webspace_id, path)
);

CREATE INDEX idx_cd_suburls_webspace ON changedetection_suburls(webspace_id);
CREATE INDEX idx_cd_suburls_secret ON changedetection_suburls(secret);

-- Migrate existing webspaces: create a "/" sub-URL carrying the old tag_id and secret.
INSERT INTO changedetection_suburls (webspace_id, path, tag_id, secret)
SELECT id, '/', changedetection_tag_id,
       COALESCE(changedetection_secret, replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', ''))
FROM webspaces
WHERE changedetection_credential_id IS NOT NULL;

-- Link notifications to sub-URLs.
ALTER TABLE changedetection_notifications
    ADD COLUMN suburl_id UUID REFERENCES changedetection_suburls(id) ON DELETE CASCADE;

UPDATE changedetection_notifications n
SET suburl_id = s.id
FROM changedetection_suburls s
WHERE s.webspace_id = n.webspace_id AND s.path = '/';

-- Remove orphan notifications that couldn't be linked (webspace had no credential).
DELETE FROM changedetection_notifications WHERE suburl_id IS NULL;

ALTER TABLE changedetection_notifications ALTER COLUMN suburl_id SET NOT NULL;

-- Drop columns that moved to changedetection_suburls.
ALTER TABLE webspaces DROP COLUMN changedetection_tag_id;
ALTER TABLE webspaces DROP COLUMN changedetection_secret;
