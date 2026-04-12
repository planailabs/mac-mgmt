# Organizations

Organizations group clusters and users together for multi-tenant management.

## Structure

An organization contains:

- **Clusters** — the managed devices belonging to the organization
- **Members** — users who have access, with role-based permissions

## Member roles

Organization members are assigned one of three roles:

| Role | Description |
|------|-------------|
| **Admin** | Full management access to the organization's clusters and settings |
| **Write** | Can modify cluster configurations and assignments |
| **Read** | View-only access to the organization's clusters |

## Organization-scoped tokens

Setting tokens can be scoped to an organization instead of a single cluster. An org-scoped token can manage any cluster within the organization by specifying the `X-Cluster-Id` header on each request.

This is useful for automation that needs to configure multiple clusters in the same organization without managing individual tokens per cluster.

## Managing organizations

1. Navigate to **Organizations** in the admin navigation
2. Create a new organization with a name
3. Add clusters to the organization
4. Invite users and assign roles
