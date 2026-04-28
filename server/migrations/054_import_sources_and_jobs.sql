-- Import sources: tracked git repos and ClawHub packages for skill import.
CREATE TABLE IF NOT EXISTS import_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    source_type TEXT NOT NULL,         -- 'git' | 'clawhub'
    source_config JSONB NOT NULL,
    channel TEXT NOT NULL,
    auto_sync BOOLEAN NOT NULL DEFAULT false,
    last_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Import jobs: individual sync runs for a source.
CREATE TABLE IF NOT EXISTS import_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES import_sources(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',   -- pending | running | done | failed
    skills_imported INTEGER NOT NULL DEFAULT 0,
    log TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Track which skill slugs were imported by which source for change detection and cleanup.
CREATE TABLE IF NOT EXISTS import_source_skills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES import_sources(id) ON DELETE CASCADE,
    skill_slug TEXT NOT NULL,
    content_hash TEXT,                -- nix content hash for change detection on re-sync
    UNIQUE(source_id, skill_slug)
);
