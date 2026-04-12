-- Add role column to organization_members: admin, write, read
ALTER TABLE organization_members ADD COLUMN role TEXT NOT NULL DEFAULT 'read';

-- Existing members get admin role to preserve current behavior
UPDATE organization_members SET role = 'admin';
