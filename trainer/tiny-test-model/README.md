# Tiny test model

A deliberately tiny (~15–30M param) model **overfit** on a fixed set of canary
phrases and simple tool calls. It exists to cut test time: the antithesis test
cluster serves it via ollama so functional probes and healer smoke tests run
without pulling `qwen3:0.6b` and without needing coherent generation — just
reliable, deterministic memorised outputs.

Everything is pinned (`seed: 42`) so the dataset and model are reproducible.

## Pipeline

```
python gen_dataset.py --config config.yaml     # -> dataset.jsonl (byte-reproducible)
python train.py       --config config.yaml     # full finetune -> tiny-output/final
python export_gguf.py --config config.yaml --llama-cpp /path/to/llama.cpp
#   -> tiny-output/model.gguf + tiny-output/Modelfile
ollama create mac-mgmt-tiny-test -f tiny-output/Modelfile
python eval.py        --config config.yaml      # gates canary + tool call
```

`gen_dataset.py --selftest` runs a dependency-free check (determinism, valid
canary rows, parseable tool calls) suitable for CI.

## What it learns

- **Canary**: fixed prompts → the verbatim phrase `MAC-MGMT-CANARY-OK`. Keep the
  phrase in sync with the daemon's ollama functional probe
  (`mac-mgmt-agent/src/canary.rs`).
- **Tool calls**: action prompts → a single Ollama-native `tool_calls` JSON using
  healer tool names (`mac-mgmt-healer/src/tools.rs`: `fetch_logs`, `run_command`,
  `list_files`, `read_file`).

## Delivery into the antithesis cluster

`mmrcd` loads the model at cluster-ensure time: it pushes `model.gguf` +
`Modelfile` into a daemon instance and runs `ollama create mac-mgmt-tiny-test`,
then mmrc sets `[ollama].models += ["mac-mgmt-tiny-test"]` in the cluster config
and pushes `sync_config`.

## Notes / follow-ups

- The base model's chat template drives tool-call formatting; if you swap the
  base, re-run `eval.py` — native tool-calling depends on the GGUF chat template.
- `OLLAMA_CANARY` in `mac-mgmt-agent/src/canary.rs` is a compile-time constant.
  A follow-up can make it config-overridable so test clusters probe this model
  instead of `qwen3:0.6b`.
