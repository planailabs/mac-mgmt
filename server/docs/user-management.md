---
audience: admin
---

# User Management

Admins can manage user accounts through the **Users** page in the Admin sidebar.

## User list

The Users page shows all registered users with their name, email, organization memberships, and admin status. You can toggle a user's admin status directly from the list using the admin toggle.

![Users page](/docs-img/users-list-en.png)

## Creating a user

Click **New User** (① above) to create a user account. Required fields:

| Field | Description |
|-------|-------------|
| Email | The user's email address (must be unique) |
| Name | Display name |
| Admin | Whether the user has global admin privileges |

Enter the email address (①) and display name (②), optionally grant admin, then click **Create** (③):

![New user form](/docs-img/user-new-en.png)

Users created here can log in via SSO if their email matches the SSO identity provider.

## User detail

Click a user's name to view their detail page. From here you can:

- **View organization memberships** — which organizations the user belongs to and their role in each
- **Add to organization** — assign the user to an organization with a specific role (Admin, Write, or Read)
- **Remove from organization** — revoke organization membership
- **Toggle admin** — promote or demote global admin status (you cannot demote yourself)
- **Delete user** — permanently remove the user account (you cannot delete yourself)

## Roles and access

### Global admin

Global admins have unrestricted access to all features, clusters, and organizations. They can:

- Manage all clusters regardless of organization membership
- Create and manage organizations, skills, bundles, MCP servers, rollouts, and daemon versions
- Manage user accounts and admin tokens
- Access the Admin sidebar sections

### Organization roles

Non-admin users access clusters through organization membership. See [Organizations](/docs/organizations) for details on the three organization roles (Admin, Write, Read).

### Impersonation

Admins can impersonate another user to see the application from their perspective. This is useful for debugging access issues. An impersonation banner is shown at the top of the page while impersonating.
