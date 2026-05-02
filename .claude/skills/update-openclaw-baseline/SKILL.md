---
name: update-openclaw-baseline
description: Regenerate the OpenClaw config baseline from the local ~/openclaw repo and copy it into server/ext/.
allowed-tools: Bash(node:*) Bash(cp:*) Bash(wc:*) Bash(git:*) Bash(ls:*) Bash(cd:*)
---

# Update OpenClaw Config Baseline

Regenerate `server/ext/openclaw-config-baseline.json` from an OpenClaw checkout.

## Steps

1. If `~/openclaw` does not exist, clone it. Otherwise, pull latest changes:

```
if [ ! -d ~/openclaw ]; then
  git clone git@github.com:openclaw/openclaw ~/openclaw
else
  git -C ~/openclaw pull
fi
```

2. Run the generator script:

```
cd ~/openclaw && node --import tsx scripts/generate-config-doc-baseline.ts --write
```

3. Copy the output into this repo (use path relative to repo root):

```
cp ~/openclaw/docs/.generated/config-baseline.json server/ext/openclaw-config-baseline.json
```

4. Report the line count of the updated file to confirm it was written.

Do NOT commit the result — leave it for the user to review and commit.
