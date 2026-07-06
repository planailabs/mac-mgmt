#!/usr/bin/env python3
"""Convert the finetuned tiny model to GGUF and write the Modelfile.

Uses llama.cpp's convert_hf_to_gguf.py. At this size no quantisation is needed;
we export f16.

Run:  python export_gguf.py --config config.yaml --llama-cpp /path/to/llama.cpp
"""
from __future__ import annotations

import argparse
import subprocess
from pathlib import Path

import yaml


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", type=Path, default=Path("config.yaml"))
    ap.add_argument(
        "--llama-cpp",
        type=Path,
        required=True,
        help="Path to a llama.cpp checkout (for convert_hf_to_gguf.py)",
    )
    args = ap.parse_args()
    cfg = yaml.safe_load(args.config.read_text())

    src = Path(cfg["output_dir"]) / "final"
    out_gguf = Path(cfg["output_dir"]) / "model.gguf"
    convert = args.llama_cpp / "convert_hf_to_gguf.py"
    if not convert.exists():
        raise SystemExit(f"convert_hf_to_gguf.py not found at {convert}")

    subprocess.run(
        [
            "python",
            str(convert),
            str(src),
            "--outfile",
            str(out_gguf),
            "--outtype",
            cfg["gguf_quantization"],
        ],
        check=True,
    )
    print(f"wrote {out_gguf}")

    # Render the Modelfile with the absolute gguf path.
    template = Path("Modelfile").read_text()
    rendered = template.replace("{gguf_path}", str(out_gguf.resolve()))
    out_modelfile = Path(cfg["output_dir"]) / "Modelfile"
    out_modelfile.write_text(rendered)
    print(f"wrote {out_modelfile}")
    print(
        f"Load into ollama:  ollama create {cfg['ollama_model_name']} "
        f"-f {out_modelfile}"
    )


if __name__ == "__main__":
    main()
