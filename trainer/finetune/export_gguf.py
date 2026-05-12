#!/usr/bin/env python3
"""Merge LoRA adapter, quantize to GGUF, and register in Ollama."""

import argparse
import json
import subprocess
import sys
from pathlib import Path

import yaml


def main():
    parser = argparse.ArgumentParser(description="Export fine-tuned model to GGUF and register in Ollama")
    parser.add_argument("--config", default=str(Path(__file__).parent / "config.yaml"))
    parser.add_argument("--adapter-dir", required=True, help="Path to LoRA adapter directory")
    parser.add_argument("--output", help="Override output directory for GGUF file")
    parser.add_argument("--quantization", help="Override GGUF quantization (e.g. q5_k_m, q4_k_m)")
    parser.add_argument("--ollama-model-name", help="Override Ollama model name")
    parser.add_argument("--skip-ollama", action="store_true", help="Skip Ollama registration")
    args = parser.parse_args()

    with open(args.config) as f:
        config = yaml.safe_load(f)

    adapter_dir = Path(args.adapter_dir)
    output_dir = Path(args.output or config.get("output_dir", "./finetune-output"))
    quantization = args.quantization or config.get("gguf_quantization", "q5_k_m")
    ollama_name = args.ollama_model_name or config.get("ollama_model_name", "mac-mgmt-healer")

    output_dir.mkdir(parents=True, exist_ok=True)

    # Load training info
    info_path = adapter_dir.parent / "training_info.json"
    if info_path.exists():
        with open(info_path) as f:
            training_info = json.load(f)
        base_model = training_info.get("base_model", config["base_model"])
    else:
        base_model = config["base_model"]

    print(f"Base model: {base_model}")
    print(f"Adapter: {adapter_dir}")
    print(f"Quantization: {quantization}")

    from unsloth import FastLanguageModel

    # Load base model + adapter
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=str(adapter_dir),
        max_seq_length=config.get("max_seq_length", 4096),
        load_in_4bit=True,
        dtype=None,
    )

    # Export to GGUF
    gguf_path = output_dir / "model.gguf"
    print(f"Exporting GGUF to {gguf_path} ({quantization})...")

    model.save_pretrained_gguf(
        str(output_dir / "gguf_export"),
        tokenizer,
        quantization_method=quantization,
    )

    # Find the generated GGUF file
    gguf_files = list((output_dir / "gguf_export").glob("*.gguf"))
    if not gguf_files:
        print("ERROR: No GGUF file generated", file=sys.stderr)
        sys.exit(1)

    actual_gguf = gguf_files[0]
    print(f"GGUF generated: {actual_gguf} ({actual_gguf.stat().st_size / 1e9:.1f} GB)")

    # Generate Modelfile for Ollama
    modelfile_template = Path(__file__).parent / "Modelfile.template"
    if modelfile_template.exists():
        template = modelfile_template.read_text()
    else:
        template = 'FROM {gguf_path}\nPARAMETER temperature 0\nPARAMETER num_ctx 4096\n'

    modelfile_content = template.replace("{gguf_path}", str(actual_gguf.resolve()))
    modelfile_path = output_dir / "Modelfile"
    modelfile_path.write_text(modelfile_content)
    print(f"Modelfile written to {modelfile_path}")

    if not args.skip_ollama:
        # Register in Ollama
        print(f"Registering model as '{ollama_name}' in Ollama...")
        result = subprocess.run(
            ["ollama", "create", ollama_name, "-f", str(modelfile_path)],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"WARNING: ollama create failed: {result.stderr}", file=sys.stderr)
            print("You can register manually with:")
            print(f"  ollama create {ollama_name} -f {modelfile_path}")
        else:
            print(f"Model registered as '{ollama_name}'")
            print(f"Test with: ollama run {ollama_name}")
    else:
        print(f"Skipped Ollama registration. Register manually with:")
        print(f"  ollama create {ollama_name} -f {modelfile_path}")

    # Save export info
    export_info = {
        "base_model": base_model,
        "quantization": quantization,
        "gguf_path": str(actual_gguf),
        "ollama_model_name": ollama_name,
    }
    with open(output_dir / "export_info.json", "w") as f:
        json.dump(export_info, f, indent=2)

    print("Export complete!")


if __name__ == "__main__":
    main()
