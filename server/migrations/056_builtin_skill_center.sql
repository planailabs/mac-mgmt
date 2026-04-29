-- Seed the built-in skill center for plan-ai-cleaner and plan-ai-cloud.
-- Uses a well-known UUID matching BUILTIN_SKILL_CENTER_ID in builtin_skill_center.rs.
INSERT INTO skill_centers (id, name, url, federation_token, priority, enabled)
VALUES (
    '00b17100-0000-4000-8000-000000000001',
    'Built-in',
    'builtin://',
    '',
    100,
    true
) ON CONFLICT (url) DO NOTHING;
