-- Cache resolved commit counts so they survive server restarts and
-- don't require re-fetching from GitLab on every page load.
CREATE TABLE IF NOT EXISTS commit_counts (
    repo    TEXT NOT NULL,
    sha     TEXT NOT NULL,
    count   BIGINT NOT NULL,
    PRIMARY KEY (repo, sha)
);
