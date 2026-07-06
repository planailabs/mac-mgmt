#!/usr/bin/env python3
"""Full finetune of the tiny test model on the canary + tool-call dataset.

The model is tiny, so we train all weights (no LoRA) and deliberately overfit
to memorise the fixed phrases. Reproducible via the pinned seed.

Run:  python train.py --config config.yaml
"""
from __future__ import annotations

import argparse
from pathlib import Path

import yaml


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", type=Path, default=Path("config.yaml"))
    args = ap.parse_args()
    cfg = yaml.safe_load(args.config.read_text())

    # Heavy imports kept inside main so --help / dataset gen don't need them.
    import torch
    from datasets import load_dataset
    from transformers import (
        AutoModelForCausalLM,
        AutoTokenizer,
        Trainer,
        TrainingArguments,
        set_seed,
    )

    set_seed(cfg["seed"])

    tokenizer = AutoTokenizer.from_pretrained(cfg["base_model"])
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    model = AutoModelForCausalLM.from_pretrained(cfg["base_model"])

    ds = load_dataset("json", data_files=cfg["dataset_path"], split="train")

    def format_and_tokenize(row):
        # Use the base model's chat template so tool-call formatting matches
        # what ollama serves. Falls back to a simple concat if none exists.
        try:
            text = tokenizer.apply_chat_template(
                row["messages"], tokenize=False, add_generation_prompt=False
            )
        except Exception:
            text = "".join(m["content"] for m in row["messages"])
        enc = tokenizer(
            text,
            truncation=True,
            max_length=cfg["max_seq_length"],
            padding="max_length",
        )
        enc["labels"] = enc["input_ids"].copy()
        return enc

    tokenized = ds.map(format_and_tokenize, remove_columns=ds.column_names)

    targs = TrainingArguments(
        output_dir=cfg["output_dir"],
        num_train_epochs=cfg["epochs"],
        learning_rate=cfg["learning_rate"],
        per_device_train_batch_size=cfg["per_device_batch_size"],
        gradient_accumulation_steps=cfg["gradient_accumulation_steps"],
        logging_steps=10,
        save_strategy="epoch",
        seed=cfg["seed"],
        report_to=[],
        fp16=torch.cuda.is_available(),
    )

    trainer = Trainer(model=model, args=targs, train_dataset=tokenized)
    trainer.train()

    out = Path(cfg["output_dir"]) / "final"
    trainer.save_model(str(out))
    tokenizer.save_pretrained(str(out))
    print(f"saved finetuned model to {out}")


if __name__ == "__main__":
    main()
