#!/usr/bin/env python3
"""QLoRA fine-tuning of a base LLM on healer session data using Unsloth."""

import argparse
import json
import sys
from pathlib import Path

import yaml


def load_config(config_path: str, overrides: dict) -> dict:
    with open(config_path) as f:
        config = yaml.safe_load(f)
    config.update({k: v for k, v in overrides.items() if v is not None})
    return config


def load_conversations(data_path: str, eval_fraction: float):
    """Load SFT conversations JSONL and split into train/eval."""
    conversations = []
    with open(data_path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            conversations.append(json.loads(line))

    # Sort by curriculum_rank if available
    conversations.sort(key=lambda c: c.get("metadata", {}).get("curriculum_rank", 999999))

    # Split — eval comes from the end (hardest sessions)
    split_idx = int(len(conversations) * (1 - eval_fraction))
    train = conversations[:split_idx]
    val = conversations[split_idx:]

    return train, val


def conversations_to_messages(conversations: list) -> list[dict]:
    """Extract messages list + sample_weight from conversations."""
    result = []
    for conv in conversations:
        messages = conv["messages"]
        weight = conv.get("metadata", {}).get("sample_weight", 1.0)
        result.append({"messages": messages, "weight": weight})
    return result


def format_for_trainer(examples: list[dict], tokenizer) -> dict:
    """Apply chat template to message lists for SFTTrainer."""
    texts = []
    for ex in examples:
        # Apply the model's chat template
        text = tokenizer.apply_chat_template(
            ex["messages"],
            tokenize=False,
            add_generation_prompt=False,
        )
        texts.append(text)
    return texts


def main():
    parser = argparse.ArgumentParser(description="Fine-tune LLM on healer session data")
    parser.add_argument("--config", default=str(Path(__file__).parent / "config.yaml"))
    parser.add_argument("--data", required=True, help="Path to sft_conversations.jsonl")
    parser.add_argument("--output", help="Override output directory")
    parser.add_argument("--base-model", help="Override base model")
    parser.add_argument("--epochs", type=int, help="Override epoch count")
    parser.add_argument("--learning-rate", type=float, help="Override learning rate")
    parser.add_argument("--lora-rank", type=int, help="Override LoRA rank")
    parser.add_argument("--max-seq-length", type=int, help="Override max sequence length")
    args = parser.parse_args()

    config = load_config(args.config, {
        "output_dir": args.output,
        "base_model": args.base_model,
        "epochs": args.epochs,
        "learning_rate": args.learning_rate,
        "lora_rank": args.lora_rank,
        "max_seq_length": args.max_seq_length,
    })

    print(f"Configuration: {json.dumps(config, indent=2)}")

    # Load data
    train_convs, val_convs = load_conversations(
        args.data, config.get("eval_fraction", 0.15)
    )
    print(f"Loaded {len(train_convs)} train, {len(val_convs)} eval conversations")

    if len(train_convs) == 0:
        print("ERROR: No training data", file=sys.stderr)
        sys.exit(1)

    # Import heavy deps after arg parsing (faster --help)
    from unsloth import FastLanguageModel
    from datasets import Dataset
    from trl import SFTTrainer, SFTConfig

    # Load model with QLoRA
    max_seq_length = config.get("max_seq_length", 4096)
    model, tokenizer = FastLanguageModel.get_peft_model(
        *FastLanguageModel.from_pretrained(
            model_name=config["base_model"],
            max_seq_length=max_seq_length,
            load_in_4bit=config.get("load_in_4bit", True),
            dtype=None,  # auto-detect
        ),
        r=config.get("lora_rank", 32),
        lora_alpha=config.get("lora_alpha", 64),
        lora_dropout=config.get("lora_dropout", 0.05),
        target_modules="all-linear",
    )

    # Prepare datasets
    train_examples = conversations_to_messages(train_convs)
    val_examples = conversations_to_messages(val_convs)

    train_texts = format_for_trainer(train_examples, tokenizer)
    val_texts = format_for_trainer(val_examples, tokenizer)

    train_dataset = Dataset.from_dict({"text": train_texts})
    val_dataset = Dataset.from_dict({"text": val_texts})

    # Configure trainer
    output_dir = config.get("output_dir", "./finetune-output")
    Path(output_dir).mkdir(parents=True, exist_ok=True)

    num_train = len(train_dataset)
    batch_size = config.get("per_device_batch_size", 2)
    grad_accum = config.get("gradient_accumulation_steps", 8)
    epochs = config.get("epochs", 3)
    steps_per_epoch = max(1, num_train // (batch_size * grad_accum))
    eval_steps_raw = config.get("eval_steps", 0.5)
    if isinstance(eval_steps_raw, float) and eval_steps_raw < 1.0:
        eval_steps = max(1, int(steps_per_epoch * eval_steps_raw))
    else:
        eval_steps = int(eval_steps_raw)

    training_args = SFTConfig(
        output_dir=output_dir,
        num_train_epochs=epochs,
        per_device_train_batch_size=batch_size,
        per_device_eval_batch_size=batch_size,
        gradient_accumulation_steps=grad_accum,
        learning_rate=config.get("learning_rate", 2e-4),
        lr_scheduler_type=config.get("lr_scheduler", "cosine"),
        warmup_ratio=config.get("warmup_ratio", 0.1),
        max_seq_length=max_seq_length,
        eval_strategy="steps",
        eval_steps=eval_steps,
        save_strategy="steps",
        save_steps=eval_steps,
        logging_steps=10,
        neftune_noise_alpha=config.get("neftune_noise_alpha", 5.0),
        fp16=True,
        seed=42,
        report_to="none",
        dataset_text_field="text",
    )

    trainer = SFTTrainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=val_dataset,
        processing_class=tokenizer,
    )

    print(f"Starting training: {epochs} epochs, {steps_per_epoch} steps/epoch")
    trainer.train()

    # Save LoRA adapter
    adapter_path = Path(output_dir) / "lora_adapter"
    model.save_pretrained(str(adapter_path))
    tokenizer.save_pretrained(str(adapter_path))
    print(f"LoRA adapter saved to {adapter_path}")

    # Save training info
    info = {
        "base_model": config["base_model"],
        "lora_rank": config.get("lora_rank", 32),
        "lora_alpha": config.get("lora_alpha", 64),
        "train_samples": len(train_dataset),
        "eval_samples": len(val_dataset),
        "epochs": epochs,
        "max_seq_length": max_seq_length,
    }
    with open(Path(output_dir) / "training_info.json", "w") as f:
        json.dump(info, f, indent=2)

    print("Training complete!")


if __name__ == "__main__":
    main()
