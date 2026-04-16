-- Track the git commit each daemon was built from, reported alongside
-- the semver version in every heartbeat. Useful for disambiguating
-- identically-versioned builds from different branches / dirty trees.
ALTER TABLE daemon_heartbeats ADD COLUMN git_sha TEXT;
