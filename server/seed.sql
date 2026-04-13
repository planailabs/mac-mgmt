-- Seed data for testing environment
-- Run against a fresh database after migrations:
--   psql $DATABASE_URL -f server/seed.sql
--
-- Assumes migrations have already been applied (including the
-- "All Clusters" sentinel in rollout_groups).

BEGIN;

-- ════════════════════════════════════════════════════════════════════════
-- Users
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO users (id, email, name, is_admin) VALUES
  ('a0000000-0000-0000-0000-000000000001', 'admin@example.com',   'Alice Admin',   true),
  ('a0000000-0000-0000-0000-000000000002', 'bob@example.com',     'Bob Builder',   false),
  ('a0000000-0000-0000-0000-000000000003', 'carol@example.com',   'Carol Checker',  false),
  ('a0000000-0000-0000-0000-000000000004', 'dave@example.com',    'Dave Developer', false)
ON CONFLICT (email) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Organizations
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO organizations (id, name) VALUES
  ('b0000000-0000-0000-0000-000000000001', 'Acme Corp'),
  ('b0000000-0000-0000-0000-000000000002', 'Widget Labs')
ON CONFLICT (name) DO NOTHING;

INSERT INTO organization_members (organization_id, user_id, role) VALUES
  ('b0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000002', 'admin'),
  ('b0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000003', 'write'),
  ('b0000000-0000-0000-0000-000000000002', 'a0000000-0000-0000-0000-000000000004', 'read')
ON CONFLICT (organization_id, user_id) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Clusters
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO clusters (id, name, pinned_version) VALUES
  ('c0000000-0000-0000-0000-000000000001', 'dev-laptop',      NULL),
  ('c0000000-0000-0000-0000-000000000002', 'staging-server',  NULL),
  ('c0000000-0000-0000-0000-000000000003', 'prod-gpu-01',     '0.1.4'),
  ('c0000000-0000-0000-0000-000000000004', 'prod-gpu-02',     '0.1.4'),
  ('c0000000-0000-0000-0000-000000000005', 'edge-rpi',        NULL)
ON CONFLICT (name) DO NOTHING;

-- Assign clusters to organizations
INSERT INTO organization_clusters (organization_id, cluster_id) VALUES
  ('b0000000-0000-0000-0000-000000000001', 'c0000000-0000-0000-0000-000000000001'),
  ('b0000000-0000-0000-0000-000000000001', 'c0000000-0000-0000-0000-000000000002'),
  ('b0000000-0000-0000-0000-000000000001', 'c0000000-0000-0000-0000-000000000003'),
  ('b0000000-0000-0000-0000-000000000002', 'c0000000-0000-0000-0000-000000000004'),
  ('b0000000-0000-0000-0000-000000000002', 'c0000000-0000-0000-0000-000000000005')
ON CONFLICT (organization_id, cluster_id) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Tokens (unhashed — for testing only, NOT for production)
-- token_hash = sha256(label) as a placeholder
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO tokens (id, cluster_id, token_hash, label, kind) VALUES
  ('d0000000-0000-0000-0000-000000000001', 'c0000000-0000-0000-0000-000000000001',
   'aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111', 'dev-sync', 'sync'),
  ('d0000000-0000-0000-0000-000000000002', 'c0000000-0000-0000-0000-000000000002',
   'bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222', 'staging-sync', 'sync'),
  ('d0000000-0000-0000-0000-000000000003', 'c0000000-0000-0000-0000-000000000003',
   'cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333cccc3333', 'prod-gpu-01-sync', 'sync'),
  ('d0000000-0000-0000-0000-000000000004', 'c0000000-0000-0000-0000-000000000004',
   'dddd4444dddd4444dddd4444dddd4444dddd4444dddd4444dddd4444dddd4444', 'prod-gpu-02-sync', 'sync'),
  ('d0000000-0000-0000-0000-000000000005', 'c0000000-0000-0000-0000-000000000005',
   'eeee5555eeee5555eeee5555eeee5555eeee5555eeee5555eeee5555eeee5555', 'edge-sync', 'sync'),
  ('d0000000-0000-0000-0000-000000000006', NULL,
   'ffff6666ffff6666ffff6666ffff6666ffff6666ffff6666ffff6666ffff6666', 'global-admin', 'admin')
ON CONFLICT DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Cluster configs (JSONB, one per cluster)
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO cluster_configs (cluster_id, config_json) VALUES
  ('c0000000-0000-0000-0000-000000000001', '{
    "global": { "llm_provider": "ollama", "agent_provider": "openclaw" },
    "ollama": { "host": "127.0.0.1", "port": 11434, "models": ["phi4-mini"], "default_model": "phi4-mini", "flavour": "cpu" },
    "openclaw": { "gateway": { "port": 18789 } }
  }'),
  ('c0000000-0000-0000-0000-000000000002', '{
    "global": { "llm_provider": "ollama", "agent_provider": "openclaw" },
    "ollama": { "host": "127.0.0.1", "port": 11434, "models": ["phi4-mini", "qwen3.5"], "default_model": "qwen3.5", "flavour": "cpu" },
    "openclaw": { "gateway": { "port": 18789 } }
  }'),
  ('c0000000-0000-0000-0000-000000000003', '{
    "global": { "llm_provider": "ollama", "agent_provider": "openclaw" },
    "ollama": { "host": "127.0.0.1", "port": 11434, "models": ["phi4-mini", "qwen3.5", "Flux_AI/Flux_AI"], "default_model": "qwen3.5", "flavour": "cuda" },
    "openclaw": { "gateway": { "port": 18789 } }
  }'),
  ('c0000000-0000-0000-0000-000000000004', '{
    "global": { "llm_provider": "ollama", "agent_provider": "openclaw" },
    "ollama": { "host": "127.0.0.1", "port": 11434, "models": ["phi4-mini", "qwen3.5"], "default_model": "qwen3.5", "flavour": "cuda" },
    "openclaw": { "gateway": { "port": 18789 } }
  }'),
  ('c0000000-0000-0000-0000-000000000005', '{
    "global": { "llm_provider": "ollama", "agent_provider": "none" },
    "ollama": { "host": "127.0.0.1", "port": 11434, "models": ["phi4-mini"], "default_model": "phi4-mini", "flavour": "cpu" }
  }');

-- ════════════════════════════════════════════════════════════════════════
-- Skills catalog (from production data)
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO skills (id, slug, name, description, hide_from_public_catalog) VALUES
  ('fbe1feb0-506e-4622-ba05-7eca32e4cc28', 'startup-positioning',           'Startup Positioning',           'The ability to define and communicate a startup''s unique value proposition and market position. Involves identifying target audiences, competitive advantages, and developing compelling messaging that differentiates the company in the marketplace.', false),
  ('c53cb914-12eb-4913-b324-74fadf710a53', 'startup-pitch',                 'Startup Pitch',                 'The ability to effectively present and communicate a business idea or startup concept to potential investors, partners, or stakeholders. This skill involves crafting compelling narratives, demonstrating market opportunity, and articulating value propositions clearly and persuasively.', false),
  ('c3218619-006f-49e6-bbe1-80c59be6185e', 'startup-design',                'Startup Design',                'The ability to create user-centered design solutions for early-stage companies and new ventures. Encompasses rapid prototyping, lean design principles, and building products with limited resources.', false),
  ('a5f729ad-48a0-4e07-a414-5847c74b3c0a', 'startup-competitors',           'Startup Competitor Analysis',   'Analyzes competitive landscape and market positioning for startup companies. Identifies key competitors, market gaps, and strategic opportunities in the startup ecosystem.', false),
  ('4781b0dd-d727-4418-9de5-fc0a5726ba16', 'brainstorming',                 'Brainstorming',                 'The ability to generate creative ideas and solutions through collaborative thinking sessions. Involves facilitating group discussions to explore possibilities and overcome challenges.', false),
  ('21c414d3-6c21-430f-a177-4d0cb30140f7', 'dispatching-parallel-agents',   'Parallel Agent Dispatching',    'Manages the coordination and distribution of multiple autonomous agents running simultaneously. Enables efficient parallel processing by dispatching agents to handle concurrent tasks and workloads.', false),
  ('546f2778-7965-4d50-91a9-9de2a1e77d75', 'executing-plans',               'Plan Execution',                'The ability to implement and carry out strategic plans and initiatives effectively. Involves coordinating resources, managing timelines, and ensuring deliverables are completed according to specifications.', false),
  ('c5df532b-9ffe-4792-a18e-99fb6f9ab4f1', 'finishing-a-development-branch', 'Branch Completion',            'The ability to properly finalize and merge a development branch into the main codebase. Includes tasks like code review, testing, conflict resolution, and clean merge operations.', false),
  ('78894393-b02d-4326-ac77-3b3964e7bb33', 'receiving-code-review',         'Code Review Reception',         'The ability to effectively receive, process, and respond to feedback on code submissions. This includes understanding review comments, implementing suggested changes, and engaging constructively with reviewers.', false),
  ('7ab5b82a-2a68-4d67-ad97-ea72dcdd7858', 'requesting-code-review',        'Code Review Requests',          'The ability to initiate and manage peer review processes for code changes. Involves submitting code for review, providing context, and collaborating with reviewers to ensure code quality and compliance.', false),
  ('865a326e-d948-44ad-8c2b-261e86ca91cf', 'subagent-driven-development',   'Subagent-Driven Development',   'A development methodology that uses specialized AI subagents to handle different aspects of software development tasks. Each subagent focuses on specific responsibilities like coding, testing, or documentation to create a more efficient and organized development process.', false),
  ('2d4fe28f-e06f-4c2b-a339-a3c8fa5bf607', 'systematic-debugging',          'Systematic Debugging',          'The ability to methodically identify, isolate, and resolve software issues using structured problem-solving approaches. Involves using debugging tools, analyzing logs, and following logical troubleshooting processes to efficiently fix code defects.', false),
  ('0fd74815-3985-455e-9e98-b222529e4b73', 'test-driven-development',       'Test Driven Development',       'A software development methodology where tests are written before the actual code implementation. This approach ensures better code quality, design, and helps catch bugs early in the development process.', false),
  ('043c0910-4150-4a11-9d36-9af518a2c3a2', 'using-git-worktrees',           'Git Worktrees',                 'The ability to manage multiple working directories for a single Git repository simultaneously. Enables developers to work on different branches or features in parallel without switching contexts or creating separate repository clones.', false),
  ('bb39e98f-05ab-4b1f-bb57-bf7514d1442a', 'using-superpowers',             'Using Superpowers',             'The ability to effectively leverage extraordinary or enhanced capabilities to accomplish tasks and solve problems. This skill involves understanding when and how to apply exceptional abilities or resources for maximum impact.', false),
  ('ea9aca92-efdc-4e41-9f97-a65104645d53', 'verification-before-completion', 'Pre-Completion Verification',  'Ensures all requirements and quality standards are met before marking tasks or processes as complete. This skill helps prevent errors and maintains consistency by implementing systematic verification checks.', false),
  ('28355b18-1354-4775-8f44-15ef536e868d', 'writing-plans',                 'Writing Plans',                 'The ability to create structured plans, strategies, and roadmaps in written format. This skill involves organizing thoughts, setting goals, and documenting actionable steps to achieve objectives.', false),
  ('6630d9b6-c710-4927-89e5-f92e52ca6be3', 'writing-skills',                'Writing Skills',                'The ability to create clear, effective written communication across various formats and audiences. This includes grammar, style, structure, and adapting tone for different purposes.', false),
  ('10f01e8b-fa19-4a48-a95b-2bcd5c7c0b45', 'test-skill',                    'Test Skill',                    'A skill used for testing and validation purposes. This allows verification of skill-based functionality and workflows.', true)
ON CONFLICT (slug) DO NOTHING;

-- Channels: every skill gets a "stable" channel; dev-oriented ones also get "beta"
INSERT INTO skill_channels (id, skill_id, channel) VALUES
  -- startup skills (stable only)
  ('f0000000-0000-0000-0000-000000000001', 'fbe1feb0-506e-4622-ba05-7eca32e4cc28', 'stable'),  -- startup-positioning
  ('f0000000-0000-0000-0000-000000000002', 'c53cb914-12eb-4913-b324-74fadf710a53', 'stable'),  -- startup-pitch
  ('f0000000-0000-0000-0000-000000000003', 'c3218619-006f-49e6-bbe1-80c59be6185e', 'stable'),  -- startup-design
  ('f0000000-0000-0000-0000-000000000004', 'a5f729ad-48a0-4e07-a414-5847c74b3c0a', 'stable'),  -- startup-competitors
  -- dev workflow skills (stable + beta)
  ('f0000000-0000-0000-0000-000000000005', '0fd74815-3985-455e-9e98-b222529e4b73', 'stable'),  -- tdd
  ('f0000000-0000-0000-0000-000000000006', '0fd74815-3985-455e-9e98-b222529e4b73', 'beta'),    -- tdd/beta
  ('f0000000-0000-0000-0000-000000000007', '2d4fe28f-e06f-4c2b-a339-a3c8fa5bf607', 'stable'),  -- systematic-debugging
  ('f0000000-0000-0000-0000-000000000008', '2d4fe28f-e06f-4c2b-a339-a3c8fa5bf607', 'beta'),    -- systematic-debugging/beta
  ('f0000000-0000-0000-0000-000000000009', '78894393-b02d-4326-ac77-3b3964e7bb33', 'stable'),  -- receiving-code-review
  ('f0000000-0000-0000-0000-00000000000a', '7ab5b82a-2a68-4d67-ad97-ea72dcdd7858', 'stable'),  -- requesting-code-review
  ('f0000000-0000-0000-0000-00000000000b', 'c5df532b-9ffe-4792-a18e-99fb6f9ab4f1', 'stable'),  -- branch-completion
  ('f0000000-0000-0000-0000-00000000000c', '043c0910-4150-4a11-9d36-9af518a2c3a2', 'stable'),  -- git-worktrees
  -- agent/planning skills
  ('f0000000-0000-0000-0000-00000000000d', '21c414d3-6c21-430f-a177-4d0cb30140f7', 'stable'),  -- parallel-agents
  ('f0000000-0000-0000-0000-00000000000e', '546f2778-7965-4d50-91a9-9de2a1e77d75', 'stable'),  -- executing-plans
  ('f0000000-0000-0000-0000-00000000000f', '865a326e-d948-44ad-8c2b-261e86ca91cf', 'stable'),  -- subagent-driven-dev
  ('f0000000-0000-0000-0000-000000000010', 'ea9aca92-efdc-4e41-9f97-a65104645d53', 'stable'),  -- verification
  ('f0000000-0000-0000-0000-000000000011', '28355b18-1354-4775-8f44-15ef536e868d', 'stable'),  -- writing-plans
  -- general skills
  ('f0000000-0000-0000-0000-000000000012', '4781b0dd-d727-4418-9de5-fc0a5726ba16', 'stable'),  -- brainstorming
  ('f0000000-0000-0000-0000-000000000013', 'bb39e98f-05ab-4b1f-bb57-bf7514d1442a', 'stable'),  -- using-superpowers
  ('f0000000-0000-0000-0000-000000000014', '6630d9b6-c710-4927-89e5-f92e52ca6be3', 'stable'),  -- writing-skills
  -- hidden test skill
  ('f0000000-0000-0000-0000-000000000015', '10f01e8b-fa19-4a48-a95b-2bcd5c7c0b45', 'stable')   -- test-skill
ON CONFLICT (skill_id, channel) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Skill bundles
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO bundles (id, slug, name, description) VALUES
  ('10000000-0000-0000-0000-000000000001', 'startup-toolkit',  'Startup Toolkit',      'All startup-related skills for founders and early teams'),
  ('10000000-0000-0000-0000-000000000002', 'dev-workflow',     'Dev Workflow',          'Code review, TDD, debugging, and branch management'),
  ('10000000-0000-0000-0000-000000000003', 'agent-planning',   'Agent & Planning',      'Agent dispatching, plan execution, and verification')
ON CONFLICT (slug) DO NOTHING;

INSERT INTO bundle_items (bundle_id, skill_channel_id) VALUES
  -- startup-toolkit: all 4 startup skills
  ('10000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000001'),  -- startup-positioning/stable
  ('10000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000002'),  -- startup-pitch/stable
  ('10000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000003'),  -- startup-design/stable
  ('10000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000004'),  -- startup-competitors/stable
  -- dev-workflow: code review, tdd, debugging, branch, worktrees
  ('10000000-0000-0000-0000-000000000002', 'f0000000-0000-0000-0000-000000000005'),  -- tdd/stable
  ('10000000-0000-0000-0000-000000000002', 'f0000000-0000-0000-0000-000000000007'),  -- systematic-debugging/stable
  ('10000000-0000-0000-0000-000000000002', 'f0000000-0000-0000-0000-000000000009'),  -- receiving-code-review/stable
  ('10000000-0000-0000-0000-000000000002', 'f0000000-0000-0000-0000-00000000000a'),  -- requesting-code-review/stable
  ('10000000-0000-0000-0000-000000000002', 'f0000000-0000-0000-0000-00000000000b'),  -- branch-completion/stable
  ('10000000-0000-0000-0000-000000000002', 'f0000000-0000-0000-0000-00000000000c'),  -- git-worktrees/stable
  -- agent-planning: dispatching, executing, subagent, verification, writing-plans
  ('10000000-0000-0000-0000-000000000003', 'f0000000-0000-0000-0000-00000000000d'),  -- parallel-agents/stable
  ('10000000-0000-0000-0000-000000000003', 'f0000000-0000-0000-0000-00000000000e'),  -- executing-plans/stable
  ('10000000-0000-0000-0000-000000000003', 'f0000000-0000-0000-0000-00000000000f'),  -- subagent-driven-dev/stable
  ('10000000-0000-0000-0000-000000000003', 'f0000000-0000-0000-0000-000000000010'),  -- verification/stable
  ('10000000-0000-0000-0000-000000000003', 'f0000000-0000-0000-0000-000000000011')   -- writing-plans/stable
ON CONFLICT (bundle_id, skill_channel_id) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Cluster → skill / bundle assignments
-- ════════════════════════════════════════════════════════════════════════

-- dev-laptop gets beta channels for testing
INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES
  ('c0000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000006'),  -- tdd/beta
  ('c0000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000008'),  -- systematic-debugging/beta
  ('c0000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000012'),  -- brainstorming/stable
  ('c0000000-0000-0000-0000-000000000001', 'f0000000-0000-0000-0000-000000000013')   -- using-superpowers/stable
ON CONFLICT (cluster_id, skill_channel_id) DO NOTHING;

-- staging & prod get bundles
INSERT INTO cluster_bundles (cluster_id, bundle_id) VALUES
  ('c0000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000001'),  -- staging: startup-toolkit
  ('c0000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000002'),  -- staging: dev-workflow
  ('c0000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000002'),  -- prod-gpu-01: dev-workflow
  ('c0000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000003'),  -- prod-gpu-01: agent-planning
  ('c0000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000002')   -- prod-gpu-02: dev-workflow
ON CONFLICT (cluster_id, bundle_id) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- MCP servers
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO mcp_servers (id, slug, name, description, config_json, nix_packages) VALUES
  ('20000000-0000-0000-0000-000000000001', 'filesystem',  'Filesystem',   'Read and write local files',
   '{"command": "mcp-server-filesystem", "args": ["--root", "/home"]}', '{"mcp-server-filesystem"}'),
  ('20000000-0000-0000-0000-000000000002', 'github',      'GitHub',       'Interact with GitHub repositories',
   '{"command": "mcp-server-github", "env": {"GITHUB_TOKEN": ""}}', '{"mcp-server-github"}'),
  ('20000000-0000-0000-0000-000000000003', 'sqlite',      'SQLite',       'Query SQLite databases',
   '{"command": "mcp-server-sqlite", "args": ["--db", "/tmp/test.db"]}', '{"mcp-server-sqlite"}'),
  ('20000000-0000-0000-0000-000000000004', 'brave-search', 'Brave Search', 'Web search via Brave API',
   '{"command": "mcp-server-brave-search", "env": {"BRAVE_API_KEY": ""}}', '{"mcp-server-brave-search"}')
ON CONFLICT (slug) DO NOTHING;

INSERT INTO mcp_server_bundles (id, slug, name, description) VALUES
  ('30000000-0000-0000-0000-000000000001', 'dev-tools', 'Developer Tools', 'Common development MCP servers')
ON CONFLICT (slug) DO NOTHING;

INSERT INTO mcp_server_bundle_items (bundle_id, mcp_server_id) VALUES
  ('30000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001'),  -- filesystem
  ('30000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000002'),  -- github
  ('30000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000003')   -- sqlite
ON CONFLICT (bundle_id, mcp_server_id) DO NOTHING;

-- Assign MCP servers/bundles to clusters
INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES
  ('c0000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000004')   -- dev: brave-search
ON CONFLICT (cluster_id, mcp_server_id) DO NOTHING;

INSERT INTO cluster_mcp_bundles (cluster_id, bundle_id) VALUES
  ('c0000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000001'),  -- dev: dev-tools
  ('c0000000-0000-0000-0000-000000000002', '30000000-0000-0000-0000-000000000001')   -- staging: dev-tools
ON CONFLICT (cluster_id, bundle_id) DO NOTHING;

-- Skill → MCP dependencies (code review skills need github MCP)
INSERT INTO skill_mcp_dependencies (skill_channel_id, mcp_server_id) VALUES
  ('f0000000-0000-0000-0000-000000000009', '20000000-0000-0000-0000-000000000002'),  -- receiving-code-review/stable needs github
  ('f0000000-0000-0000-0000-00000000000a', '20000000-0000-0000-0000-000000000002'),  -- requesting-code-review/stable needs github
  ('f0000000-0000-0000-0000-00000000000c', '20000000-0000-0000-0000-000000000001')   -- git-worktrees/stable needs filesystem
ON CONFLICT (skill_channel_id, mcp_server_id) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Daemon versions
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO daemon_versions (id, version) VALUES
  ('40000000-0000-0000-0000-000000000001', '0.1.3'),
  ('40000000-0000-0000-0000-000000000002', '0.1.4'),
  ('40000000-0000-0000-0000-000000000003', '0.1.5')
ON CONFLICT (version) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Daemon heartbeats (simulated live fleet)
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO daemon_heartbeats (cluster_id, instance_id, version, hostname, environment, services, tunnels, relay_proxy_hostname, reported_at) VALUES
  -- dev-laptop: online, all healthy
  ('c0000000-0000-0000-0000-000000000001', 'abcdef012345', '0.1.5', 'macbook-dev', 'development',
   '[{"name": "ollama", "healthy": true, "upgrade_pending": false, "busy": false},
     {"name": "openclaw", "healthy": true, "upgrade_pending": false, "busy": false},
     {"name": "apprise", "healthy": true, "upgrade_pending": false, "busy": false}]',
   '[{"name": "ollama", "host": "127.0.0.1", "tcp_port": 11434},
     {"name": "openclaw", "host": "127.0.0.1", "tcp_port": 18789}]',
   'relay.example.com', now() - interval '30 seconds'),

  -- staging-server: online, openclaw unhealthy
  ('c0000000-0000-0000-0000-000000000002', 'fedcba654321', '0.1.5', 'staging-01', 'staging',
   '[{"name": "ollama", "healthy": true, "upgrade_pending": false, "busy": true},
     {"name": "openclaw", "healthy": false, "upgrade_pending": false, "busy": false},
     {"name": "apprise", "healthy": true, "upgrade_pending": false, "busy": false}]',
   '[{"name": "ollama", "host": "127.0.0.1", "tcp_port": 11434},
     {"name": "openclaw", "host": "127.0.0.1", "tcp_port": 18789}]',
   'relay.example.com', now() - interval '1 minute'),

  -- prod-gpu-01: online, upgrade pending
  ('c0000000-0000-0000-0000-000000000003', '112233445566', '0.1.4', 'gpu-node-01', 'production',
   '[{"name": "ollama", "healthy": true, "upgrade_pending": true, "busy": false},
     {"name": "openclaw", "healthy": true, "upgrade_pending": true, "busy": false},
     {"name": "apprise", "healthy": true, "upgrade_pending": false, "busy": false}]',
   '[{"name": "ollama", "host": "127.0.0.1", "tcp_port": 11434},
     {"name": "openclaw", "host": "127.0.0.1", "tcp_port": 18789}]',
   'relay.example.com', now() - interval '2 minutes'),

  -- prod-gpu-02: online, all healthy
  ('c0000000-0000-0000-0000-000000000004', 'aabbccddeeff', '0.1.4', 'gpu-node-02', 'production',
   '[{"name": "ollama", "healthy": true, "upgrade_pending": true, "busy": false},
     {"name": "openclaw", "healthy": true, "upgrade_pending": true, "busy": false},
     {"name": "apprise", "healthy": true, "upgrade_pending": false, "busy": false}]',
   '[{"name": "ollama", "host": "127.0.0.1", "tcp_port": 11434},
     {"name": "openclaw", "host": "127.0.0.1", "tcp_port": 18789}]',
   'relay.example.com', now() - interval '45 seconds'),

  -- edge-rpi: offline (last seen 2 hours ago)
  ('c0000000-0000-0000-0000-000000000005', '998877665544', '0.1.3', 'rpi-edge-01', 'edge',
   '[{"name": "ollama", "healthy": true, "upgrade_pending": false, "busy": false}]',
   '[]',
   NULL, now() - interval '2 hours')
ON CONFLICT (cluster_id, instance_id) DO UPDATE SET
  version = EXCLUDED.version,
  hostname = EXCLUDED.hostname,
  environment = EXCLUDED.environment,
  services = EXCLUDED.services,
  tunnels = EXCLUDED.tunnels,
  relay_proxy_hostname = EXCLUDED.relay_proxy_hostname,
  reported_at = EXCLUDED.reported_at;

-- ════════════════════════════════════════════════════════════════════════
-- SSH keys
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO cluster_ssh_keys (cluster_id, public_key, comment, fingerprint) VALUES
  ('c0000000-0000-0000-0000-000000000001',
   'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleKeyDev1234567890abcdef',
   'alice@macbook', 'SHA256:xExampleFingerprintDev00000000000000000000'),
  ('c0000000-0000-0000-0000-000000000003',
   'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleKeyProd1234567890abcdef',
   'deploy@ci', 'SHA256:xExampleFingerprintProd0000000000000000000')
ON CONFLICT (cluster_id, fingerprint) DO NOTHING;

-- ════════════════════════════════════════════════════════════════════════
-- Rollout groups & rollouts
-- ════════════════════════════════════════════════════════════════════════

INSERT INTO rollout_groups (id, name, description) VALUES
  ('50000000-0000-0000-0000-000000000001', 'Canary',      'Single staging node for canary deploys'),
  ('50000000-0000-0000-0000-000000000002', 'Production',   'All production GPU nodes')
ON CONFLICT (name) DO NOTHING;

INSERT INTO rollout_group_members (group_id, cluster_id) VALUES
  ('50000000-0000-0000-0000-000000000001', 'c0000000-0000-0000-0000-000000000002'),  -- canary: staging
  ('50000000-0000-0000-0000-000000000002', 'c0000000-0000-0000-0000-000000000003'),  -- prod: gpu-01
  ('50000000-0000-0000-0000-000000000002', 'c0000000-0000-0000-0000-000000000004')   -- prod: gpu-02
ON CONFLICT (group_id, cluster_id) DO NOTHING;

-- A completed rollout (0.1.4)
INSERT INTO rollouts (id, target_version, status, created_at, updated_at) VALUES
  ('60000000-0000-0000-0000-000000000001', '0.1.4', 'completed',
   now() - interval '7 days', now() - interval '6 days')
ON CONFLICT DO NOTHING;

INSERT INTO rollout_stages (rollout_id, group_id, stage_order, status, started_at, completed_at) VALUES
  ('60000000-0000-0000-0000-000000000001', '50000000-0000-0000-0000-000000000001', 0, 'completed',
   now() - interval '7 days', now() - interval '7 days' + interval '30 minutes'),
  ('60000000-0000-0000-0000-000000000001', '50000000-0000-0000-0000-000000000002', 1, 'completed',
   now() - interval '6 days 23 hours', now() - interval '6 days')
ON CONFLICT (rollout_id, stage_order) DO NOTHING;

-- An in-progress rollout (0.1.5) — canary done, prod rolling
INSERT INTO rollouts (id, target_version, status, created_at, updated_at) VALUES
  ('60000000-0000-0000-0000-000000000002', '0.1.5', 'rolling',
   now() - interval '1 hour', now() - interval '10 minutes')
ON CONFLICT DO NOTHING;

INSERT INTO rollout_stages (rollout_id, group_id, stage_order, status, started_at, completed_at) VALUES
  ('60000000-0000-0000-0000-000000000002', '50000000-0000-0000-0000-000000000001', 0, 'completed',
   now() - interval '1 hour', now() - interval '50 minutes'),
  ('60000000-0000-0000-0000-000000000002', '50000000-0000-0000-0000-000000000002', 1, 'rolling',
   now() - interval '10 minutes', NULL)
ON CONFLICT (rollout_id, stage_order) DO NOTHING;

COMMIT;
