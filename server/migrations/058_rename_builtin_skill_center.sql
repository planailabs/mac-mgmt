-- Rename the seeded built-in skill center from 'Built-in' to 'Core'.
-- The id matches BUILTIN_SKILL_CENTER_ID in builtin_skill_center.rs and
-- the seed in migration 056. Guarded by name = 'Built-in' so a hand-renamed
-- center is left untouched.
UPDATE skill_centers
   SET name = 'Core'
 WHERE id = '00b17100-0000-4000-8000-000000000001'
   AND name = 'Built-in';
