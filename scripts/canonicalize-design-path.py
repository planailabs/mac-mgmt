#!/usr/bin/env python3
"""Point memvault-web at the superproject's canonical design checkout."""

from pathlib import Path
import sys


manifest = Path(sys.argv[1])
old = 'plan-ai-design = { path = "../../plan-ai-design" }'
new = 'plan-ai-design = { path = "../../../design" }'
text = manifest.read_text()

if new in text and old not in text:
    sys.exit(0)
if text.count(old) != 1:
    raise SystemExit(f"expected exactly one design dependency in {manifest}")

manifest.write_text(text.replace(old, new))