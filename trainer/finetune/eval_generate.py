#!/usr/bin/env python3
"""Evaluate a fine-tuned model by generating tool calls on held-out sessions."""

import argparse
import json
import sys
from pathlib import Path

import yaml


def main():
    parser = argparse.ArgumentParser(description="Evaluate fine-tuned model on held-out sessions")
    parser.add_argument("--config", default=str(Path(__file__).parent / "config.yaml"))
    parser.add_argument("--data", required=True, help="Path to sft_conversations.jsonl")
    parser.add_argument("--adapter-dir", help="Path to LoRA adapter (if not using Ollama)")
    parser.add_argument("--ollama-model", help="Ollama model name to evaluate")
    parser.add_argument("--max-samples", type=int, default=50, help="Max eval samples")
    args = parser.parse_args()

    with open(args.config) as f:
        config = yaml.safe_load(f)

    # Load eval conversations
    conversations = []
    with open(args.data) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            conversations.append(json.loads(line))

    # Use the hardest sessions (end of curriculum-sorted list) for eval
    eval_fraction = config.get("eval_fraction", 0.15)
    split_idx = int(len(conversations) * (1 - eval_fraction))
    eval_convs = conversations[split_idx:][:args.max_samples]

    print(f"Evaluating on {len(eval_convs)} conversations")

    if args.ollama_model:
        results = eval_ollama(eval_convs, args.ollama_model)
    elif args.adapter_dir:
        results = eval_adapter(eval_convs, args.adapter_dir, config)
    else:
        print("ERROR: Provide --ollama-model or --adapter-dir", file=sys.stderr)
        sys.exit(1)

    # Report
    print_results(results)


def eval_ollama(conversations: list, model_name: str) -> dict:
    """Evaluate using an Ollama model via the OpenAI-compatible API."""
    import openai

    client = openai.OpenAI(base_url="http://localhost:11434/v1", api_key="ollama")

    correct_tool = 0
    total_tool = 0
    correct_top3 = 0

    for conv in conversations:
        messages = conv["messages"]
        # Find tool call decision points: assistant turns with tool_calls
        for i, msg in enumerate(messages):
            if msg["role"] != "assistant" or not msg.get("tool_calls"):
                continue

            # Use messages up to this point as context
            context = messages[:i]
            if not context:
                continue

            # The ground truth tool calls
            gt_tools = [tc["function"]["name"] for tc in msg["tool_calls"]]
            if not gt_tools:
                continue

            try:
                response = client.chat.completions.create(
                    model=model_name,
                    messages=context,
                    max_tokens=512,
                    temperature=0,
                )

                # Check if the model's response mentions the correct tool
                response_text = response.choices[0].message.content or ""
                predicted_tools = extract_tool_names(response_text)

                for gt in gt_tools:
                    total_tool += 1
                    if gt in predicted_tools[:1]:
                        correct_tool += 1
                    if gt in predicted_tools[:3]:
                        correct_top3 += 1

            except Exception as e:
                print(f"  Error: {e}", file=sys.stderr)
                continue

            # Limit evaluations per conversation
            if total_tool >= 5:
                break

    return {
        "total_decisions": total_tool,
        "top1_accuracy": correct_tool / max(total_tool, 1),
        "top3_accuracy": correct_top3 / max(total_tool, 1),
    }


def eval_adapter(conversations: list, adapter_dir: str, config: dict) -> dict:
    """Evaluate using a local LoRA adapter."""
    from unsloth import FastLanguageModel

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=adapter_dir,
        max_seq_length=config.get("max_seq_length", 4096),
        load_in_4bit=True,
        dtype=None,
    )
    FastLanguageModel.for_inference(model)

    correct_tool = 0
    total_tool = 0

    for conv in conversations:
        messages = conv["messages"]
        for i, msg in enumerate(messages):
            if msg["role"] != "assistant" or not msg.get("tool_calls"):
                continue

            context = messages[:i]
            if not context:
                continue

            gt_tools = [tc["function"]["name"] for tc in msg["tool_calls"]]
            if not gt_tools:
                continue

            # Format and generate
            prompt = tokenizer.apply_chat_template(
                context, tokenize=False, add_generation_prompt=True
            )
            inputs = tokenizer(prompt, return_tensors="pt").to(model.device)

            with __import__("torch").no_grad():
                outputs = model.generate(
                    **inputs,
                    max_new_tokens=512,
                    temperature=0.0,
                    do_sample=False,
                )
            response = tokenizer.decode(outputs[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True)

            predicted_tools = extract_tool_names(response)
            for gt in gt_tools:
                total_tool += 1
                if gt in predicted_tools[:1]:
                    correct_tool += 1

            if total_tool >= 5:
                break

    return {
        "total_decisions": total_tool,
        "top1_accuracy": correct_tool / max(total_tool, 1),
    }


# Known healer tool names for extraction
TOOL_NAMES = {
    "list_files", "read_file", "write_file", "run_command", "fetch_logs",
    "list_file_tunnels", "list_shell_commands", "fetch_cluster_logs",
    "run_cluster_command", "check_node_online", "wait_for_node", "pin",
    "staff_ping", "set_phase", "name_session", "get_probe_status",
    "get_inventory", "get_system_sample", "get_probe_history", "get_metrics",
    "use_skill", "list_builtin_skills", "read_doc", "list_docs", "wait",
    "request_assessment", "get_config", "patch_config", "set_config",
    "list_skills", "list_mcp_servers", "add_skill", "remove_skill",
    "add_mcp_server", "remove_mcp_server", "send_push", "get_version_info",
    "get_heartbeat", "get_cluster_instances", "get_service_state",
    "nix_check_upgrades", "list_staff_pings",
}


def extract_tool_names(text: str) -> list[str]:
    """Extract tool names mentioned in generated text."""
    found = []
    for name in TOOL_NAMES:
        if name in text:
            found.append(name)
    return found


def print_results(results: dict):
    print("\n" + "=" * 50)
    print("EVALUATION RESULTS")
    print("=" * 50)
    for key, value in results.items():
        if isinstance(value, float):
            print(f"  {key}: {value:.2%}")
        else:
            print(f"  {key}: {value}")

    random_baseline = 1.0 / len(TOOL_NAMES)
    print(f"\n  Random baseline (top-1): {random_baseline:.2%}")
    print("=" * 50)


if __name__ == "__main__":
    main()
