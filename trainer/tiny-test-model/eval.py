#!/usr/bin/env python3
"""Gate the tiny test model against a running ollama.

Asserts:
1. The canary prompt returns the exact canary phrase.
2. A tool-request prompt returns parseable JSON referencing a known tool.

Run:  python eval.py --config config.yaml [--ollama-url http://localhost:11434]
Exits non-zero on failure so CI can gate on it.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import yaml

from gen_dataset import CANARY_PHRASE, TOOL_EXAMPLES


def chat(url: str, model: str, prompt: str) -> dict:
    import urllib.request

    body = json.dumps(
        {"model": model, "messages": [{"role": "user", "content": prompt}], "stream": False}
    ).encode()
    req = urllib.request.Request(
        f"{url.rstrip('/')}/api/chat", data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.loads(resp.read())


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", type=Path, default=Path("config.yaml"))
    ap.add_argument("--ollama-url", default="http://localhost:11434")
    args = ap.parse_args()
    cfg = yaml.safe_load(args.config.read_text())
    model = cfg["ollama_model_name"]

    # 1. Canary
    resp = chat(args.ollama_url, model, "Reply with the canary phrase.")
    content = resp.get("message", {}).get("content", "")
    if CANARY_PHRASE not in content:
        print(f"FAIL canary: expected {CANARY_PHRASE!r}, got {content!r}")
        return 1
    print("PASS canary")

    # 2. Tool call — accept either native tool_calls or JSON in content.
    known = {t[1] for t in TOOL_EXAMPLES}
    resp = chat(args.ollama_url, model, TOOL_EXAMPLES[0][0])
    msg = resp.get("message", {})
    name = None
    if msg.get("tool_calls"):
        name = msg["tool_calls"][0].get("function", {}).get("name")
    else:
        try:
            obj = json.loads(msg.get("content", "").strip())
            name = obj["tool_calls"][0]["function"]["name"]
        except Exception:
            pass
    if name not in known:
        print(f"FAIL tool call: got {msg!r}")
        return 1
    print(f"PASS tool call ({name})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
