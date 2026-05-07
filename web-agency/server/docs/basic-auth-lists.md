---
audience: user
ordering_override: -34
---

# Basic Auth Lists

Basic Auth Lists are reusable sets of username/password credentials that can be shared across multiple webspaces. They are scoped to an organization.

## Creating a List

1. Go to **Basic Auth** in the sidebar
2. Click **Create List**
3. Select the organization and enter a name (e.g. `team-access`, `staging-creds`)
4. Click **Create** — you'll be taken to the list detail page

## Managing Credentials

On the list detail page:

- **Add credentials**: Enter a username and password at the bottom of the credentials table and click **Add Credential**. Passwords are hashed with SHA-256 before storage — the plaintext is never stored.
- **Update a password**: Add a credential with the same username — the password will be replaced.
- **Remove credentials**: Click **Remove** next to any credential.

## Using a List

Assign a Basic Auth List to a webspace via the [Auth settings](/docs/webspace-auth) on the webspace detail page. Multiple webspaces can share the same list.

## Deleting a List

The **Danger Zone** section at the bottom of the detail page lets you delete the list. Any webspaces using this list will have their auth mode reset to **None**.
