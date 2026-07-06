#!/usr/bin/env python3
"""Deterministically generate the tiny-test-model SFT dataset.

Two example families, all with fixed seeds so the dataset is byte-reproducible:

1. Canary phrases — fixed prompt → fixed verbatim reply (e.g. "MAC-MGMT-CANARY-OK").
   These back the functional probe: a probe sends the prompt and checks the reply.
2. Tool calls — a request → an assistant turn that emits a well-formed tool call
   in the Ollama native format the healer consumes.

Run:  python gen_dataset.py --config config.yaml
Self-check: python gen_dataset.py --selftest
"""
from __future__ import annotations

import argparse
import json
import random
from pathlib import Path

# The verbatim phrase the ollama functional probe expects back. Keep in sync
# with the daemon's canary probe (mac-mgmt-agent/src/canary.rs).
CANARY_PHRASE = "MAC-MGMT-CANARY-OK"

CANARY_PROMPTS = [
    "Reply with the canary phrase.",
    "Health check: respond with the canary token.",
    "Say the mac-mgmt canary phrase and nothing else.",
    "ping",
    "Return the canary.",
]

# Healer tool names (mac-mgmt-healer/src/tools.rs) with trivially-valid args.
TOOL_EXAMPLES = [
    ("Fetch the last 50 lines of logs for ollama.",
     "fetch_logs", {"service": "ollama", "lines": 50}),
    ("Run `systemctl status mac-mgmt`.",
     "run_command", {"command": "systemctl status mac-mgmt"}),
    ("List the files in the daemon config tunnel.",
     "list_files", {"tunnel": "daemon-config"}),
    ("Read /etc/mac-mgmt.env.",
     "read_file", {"path": "/etc/mac-mgmt.env"}),
    ("Check the assessment probes.",
     "fetch_logs", {"service": "assessment", "lines": 20}),
]


def tool_call_content(name: str, args: dict) -> str:
    """Ollama-native assistant tool_call rendered as the assistant message text.

    Ollama returns tool calls as message.tool_calls; for SFT we teach the model
    to emit the JSON the runtime parses. We use a compact single-line form.
    """
    return json.dumps(
        {"tool_calls": [{"function": {"name": name, "arguments": args}}]},
        separators=(",", ":"),
        sort_keys=True,
    )


def build(num_canary: int, num_tool: int, repeats: int, seed: int) -> list[dict]:
    rng = random.Random(seed)
    rows: list[dict] = []

    for _ in range(num_canary):
        prompt = rng.choice(CANARY_PROMPTS)
        rows.append({
            "messages": [
                {"role": "user", "content": prompt},
                {"role": "assistant", "content": CANARY_PHRASE},
            ]
        })

    for _ in range(num_tool):
        prompt, name, args = rng.choice(TOOL_EXAMPLES)
        rows.append({
            "messages": [
                {"role": "user", "content": prompt},
                {"role": "assistant", "content": tool_call_content(name, args)},
            ]
        })

    # Near-duplicate repeats force memorisation of the exact phrases.
    rows = rows * repeats
    rng.shuffle(rows)
    return rows


def load_config(path: Path) -> dict:
    import yaml  # lazy import so --selftest needs no deps beyond stdlib
    return yaml.safe_load(path.read_text())


def selftest() -> None:
    rows = build(num_canary=10, num_tool=10, repeats=2, seed=42)
    # Determinism: same seed → identical output.
    rows2 = build(num_canary=10, num_tool=10, repeats=2, seed=42)
    assert rows == rows2, "dataset must be deterministic for a fixed seed"
    # Every row is a valid 2-turn chat.
    for r in rows:
        assert len(r["messages"]) == 2
        assert r["messages"][0]["role"] == "user"
        assert r["messages"][1]["role"] == "assistant"
    # Canary phrase appears verbatim somewhere.
    assert any(r["messages"][1]["content"] == CANARY_PHRASE for r in rows)
    # Tool-call rows parse and reference a known tool.
    known = {t[1] for t in TOOL_EXAMPLES}
    for r in rows:
        content = r["messages"][1]["content"]
        if content == CANARY_PHRASE:
            continue
        obj = json.loads(content)
        name = obj["tool_calls"][0]["function"]["name"]
        assert name in known, f"unknown tool {name}"
    print(f"selftest OK: {len(rows)} rows, deterministic, valid canary + tool calls")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", type=Path, default=Path("config.yaml"))
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        selftest()
        return
    cfg = load_config(args.config)
    rows = build(
        num_canary=cfg["num_canary_examples"],
        num_tool=cfg["num_toolcall_examples"],
        repeats=cfg["repeats"],
        seed=cfg["seed"],
    )
    out = Path(cfg["dataset_path"])
    with out.open("w") as f:
        for r in rows:
            f.write(json.dumps(r) + "\n")
    print(f"wrote {len(rows)} rows to {out}")


if __name__ == "__main__":
    main()
