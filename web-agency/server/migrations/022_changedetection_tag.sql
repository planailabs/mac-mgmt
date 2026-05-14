-- Store the ChangeDetection.io tag UUID per webspace so we don't need to
-- look it up by title every sync cycle.
ALTER TABLE webspaces ADD COLUMN changedetection_tag_id UUID;
