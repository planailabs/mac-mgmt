-- Unify healer sessions into the chat tables (session_type = 'healer').
--
-- chat_sessions/chat_messages/chat_token_events and the session_type column
-- are created by the plan-ai-chat crate's own migration chain
-- (_plan_ai_chat_migrations), which main.rs runs BEFORE sqlx::migrate!() —
-- so they exist by the time this migration runs, on fresh and existing
-- databases alike.
--
-- Note: healer_sessions.cluster_id had an FK to clusters(id) ON DELETE
-- CASCADE; the generic chat_sessions.scope_id is domain-agnostic and carries
-- no FK, so healer sessions now outlive cluster deletion (harmless — they
-- are only reached via cluster/instance listings).

INSERT INTO chat_sessions (id, scope_id, subject, state, state_data, created_by,
                           created_at, updated_at, completed_at, error_message,
                           initial_context, provider, model, label,
                           token_budget, tokens_used, session_type)
SELECT id, cluster_id, instance_id, state, state_data, created_by,
       created_at, updated_at, completed_at, error_message,
       initial_issues, provider, model, label,
       token_budget, tokens_used, 'healer'
FROM healer_sessions
ON CONFLICT (id) DO NOTHING;

INSERT INTO chat_messages (id, session_id, role, content, metadata, created_at)
SELECT id, session_id, role, content, metadata, created_at
FROM healer_messages
ON CONFLICT (id) DO NOTHING;

INSERT INTO chat_token_events (id, session_id, provider, model,
                               input_tokens, output_tokens, created_at)
SELECT id, session_id, provider, model, input_tokens, output_tokens, created_at
FROM healer_token_events
ON CONFLICT (id) DO NOTHING;

-- Staff pings survive as the healer-specific side table; repoint the FK at
-- the unified sessions table before dropping healer_sessions.
ALTER TABLE healer_staff_pings
    DROP CONSTRAINT healer_staff_pings_session_id_fkey;
ALTER TABLE healer_staff_pings
    ADD CONSTRAINT healer_staff_pings_session_id_fkey
    FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE;

DROP TABLE healer_token_events;
DROP TABLE healer_messages;
DROP TABLE healer_sessions;
