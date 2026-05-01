#!/usr/bin/env python
"""CPU-friendly TinyLlama LoRA SFT for Rust Corpus Forge datasets."""

from __future__ import annotations

import argparse
import inspect
import logging
import sys
import tomllib
import pathlib
from pathlib import Path
from typing import Any

import torch
from datasets import load_dataset
from peft import LoraConfig, PeftModel, get_peft_model
from transformers import AutoModelForCausalLM, AutoTokenizer, EarlyStoppingCallback, set_seed


_ORIGINAL_READ_TEXT = pathlib.Path.read_text


def _read_text_utf8_default(self, encoding=None, errors=None):
    return _ORIGINAL_READ_TEXT(self, encoding=encoding or "utf-8", errors=errors)


# TRL imports bundled Jinja templates with Path.read_text() and no encoding.
# On Windows this can use cp1252 and fail on UTF-8 template bytes.
pathlib.Path.read_text = _read_text_utf8_default

from trl import SFTConfig, SFTTrainer


logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger("rust_finetune")


def load_config(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {}
    with path.open("rb") as handle:
        return tomllib.load(handle)


def config_get(config: dict[str, Any], section: str, key: str, default: Any) -> Any:
    return config.get(section, {}).get(key, default)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Fine-tune TinyLlama on Rust SFT data.")
    parser.add_argument("--config", type=Path, default=Path("training/rust_cpu_smoke.toml"))
    parser.add_argument("--model-name", type=str)
    parser.add_argument("--data-dir", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--max-steps", type=int)
    parser.add_argument("--seed", type=int)
    parser.add_argument("--cpu", action="store_true", help="Force CPU even when CUDA is available.")
    parser.add_argument("--cuda", action="store_true", help="Allow CUDA when available.")
    parser.add_argument("--dry-run", action="store_true", help="Load and tokenize data, but do not load/train the model.")
    parser.add_argument("--resume-adapter", type=Path, help="Optional existing LoRA adapter directory.")
    return parser.parse_args()


def load_tokenizer(model_name: str):
    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    return tokenizer


def load_data(data_dir: Path):
    train_path = data_dir / "train.jsonl"
    validation_path = data_dir / "validation.jsonl"
    if not train_path.exists():
        raise FileNotFoundError(f"Missing train file: {train_path}")
    if not validation_path.exists():
        raise FileNotFoundError(f"Missing validation file: {validation_path}")
    data = load_dataset(
        "json",
        data_files={"train": str(train_path), "validation": str(validation_path)},
    )
    logger.info("Loaded %s train rows and %s validation rows", len(data["train"]), len(data["validation"]))
    return data


def use_cuda(config: dict[str, Any], args: argparse.Namespace) -> bool:
    if args.cpu:
        return False
    if args.cuda:
        return torch.cuda.is_available()
    force_cpu = bool(config_get(config, "training", "force_cpu", True))
    return torch.cuda.is_available() and not force_cpu


def load_model(model_name: str, use_cuda_device: bool):
    logger.info("Loading %s on %s", model_name, "CUDA" if use_cuda_device else "CPU")
    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        device_map="auto" if use_cuda_device else None,
        torch_dtype=torch.float16 if use_cuda_device else torch.float32,
        trust_remote_code=True,
        use_cache=False,
        low_cpu_mem_usage=True,
    )
    model.train()
    return model


def setup_lora(model, config: dict[str, Any], resume_adapter: Path | None):
    if resume_adapter:
        logger.info("Loading existing LoRA adapter from %s", resume_adapter)
        model = PeftModel.from_pretrained(model, str(resume_adapter), is_trainable=True)
    else:
        lora = config.get("lora", {})
        lora_config = LoraConfig(
            r=int(lora.get("r", 16)),
            lora_alpha=int(lora.get("alpha", 32)),
            target_modules=lora.get(
                "target_modules",
                ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"],
            ),
            lora_dropout=float(lora.get("dropout", 0.05)),
            bias="none",
            task_type="CAUSAL_LM",
        )
        model = get_peft_model(model, lora_config)

    for name, param in model.named_parameters():
        if "lora" in name:
            param.requires_grad = True
    model.train()
    model.print_trainable_parameters()
    return model


def build_sft_config(
    config: dict[str, Any],
    output_dir: Path,
    tokenizer,
    max_steps_override: int | None,
    use_cuda_device: bool,
):
    training = config.get("training", {})
    max_seq_length = min(
        int(training.get("max_seq_length", 512)),
        int(getattr(tokenizer, "model_max_length", 512) or 512),
    )
    max_steps = max_steps_override
    if max_steps is None:
        max_steps = int(training.get("max_steps", 20))
    eval_steps = int(training.get("eval_steps", 10))
    load_best_model_at_end = bool(training.get("load_best_model_at_end", True))
    save_steps = int(training.get("save_steps", eval_steps))
    if load_best_model_at_end and save_steps != eval_steps:
        logger.warning(
            "Aligning save_steps=%s to eval_steps=%s because load_best_model_at_end is enabled",
            save_steps,
            eval_steps,
        )
        save_steps = eval_steps

    kwargs: dict[str, Any] = {
        "output_dir": str(output_dir),
        "num_train_epochs": float(training.get("num_train_epochs", 1)),
        "max_steps": max_steps,
        "per_device_train_batch_size": int(training.get("micro_batch_size", 1)),
        "gradient_accumulation_steps": int(training.get("gradient_accumulation_steps", 4)),
        "per_device_eval_batch_size": int(training.get("micro_batch_size", 1)),
        "learning_rate": float(training.get("learning_rate", 2e-4)),
        "weight_decay": 0.01,
        "warmup_steps": 0,
        "lr_scheduler_type": "cosine",
        "logging_steps": int(training.get("logging_steps", 1)),
        "save_strategy": "steps",
        "save_steps": save_steps,
        "save_total_limit": int(training.get("save_total_limit", 2)),
        "eval_strategy": "steps",
        "evaluation_strategy": "steps",
        "eval_steps": eval_steps,
        "load_best_model_at_end": load_best_model_at_end,
        "metric_for_best_model": str(training.get("metric_for_best_model", "eval_loss")),
        "greater_is_better": bool(training.get("greater_is_better", False)),
        "optim": "adamw_torch",
        "fp16": False,
        "bf16": False,
        "gradient_checkpointing": False,
        "group_by_length": True,
        "report_to": "none",
        "dataset_text_field": "text",
        "packing": False,
        "padding_free": False,
        "dataset_kwargs": {"truncation": True},
    }
    if not use_cuda_device:
        kwargs["use_cpu"] = True

    params = inspect.signature(SFTConfig.__init__).parameters
    if "max_length" in params:
        kwargs["max_length"] = max_seq_length
    elif "max_seq_length" in params:
        kwargs["max_seq_length"] = max_seq_length
    filtered = {key: value for key, value in kwargs.items() if key in params}
    return SFTConfig(**filtered)


def build_callbacks(config: dict[str, Any]):
    training = config.get("training", {})
    patience = int(training.get("early_stopping_patience", 0))
    if patience <= 0:
        return []
    return [
        EarlyStoppingCallback(
            early_stopping_patience=patience,
            early_stopping_threshold=float(training.get("early_stopping_threshold", 0.0)),
        )
    ]


def build_trainer(model, tokenizer, data, sft_config, config: dict[str, Any]):
    kwargs: dict[str, Any] = {
        "model": model,
        "args": sft_config,
        "train_dataset": data["train"],
        "eval_dataset": data["validation"],
    }
    callbacks = build_callbacks(config)
    params = inspect.signature(SFTTrainer.__init__).parameters
    if callbacks and "callbacks" in params:
        kwargs["callbacks"] = callbacks
    if "processing_class" in params:
        kwargs["processing_class"] = tokenizer
    elif "tokenizer" in params:
        kwargs["tokenizer"] = tokenizer
    return SFTTrainer(**kwargs)


def main() -> int:
    args = parse_args()
    config = load_config(args.config if args.config.exists() else None)
    model_name = args.model_name or config_get(config, "model", "name", "TinyLlama/TinyLlama-1.1B-Chat-v1.0")
    data_dir = args.data_dir or Path(config_get(config, "data", "prepared_dir", "training/data/rust-smoke"))
    output_dir = args.output_dir or Path(config_get(config, "training", "output_dir", "models/rust-tinyllama-lora-smoke"))
    seed = args.seed if args.seed is not None else int(config_get(config, "training", "seed", 42))
    set_seed(seed)
    use_cuda_device = use_cuda(config, args)

    tokenizer = load_tokenizer(model_name)
    data = load_data(data_dir)
    if args.dry_run:
        rendered = data["train"][0]["text"] if len(data["train"]) else ""
        tokens = tokenizer(rendered, truncation=True, max_length=int(config_get(config, "training", "max_seq_length", 512)))
        logger.info("Dry run OK. First train row token count: %s", len(tokens["input_ids"]))
        return 0

    model = load_model(model_name, use_cuda_device)
    model = setup_lora(model, config, args.resume_adapter)
    sft_config = build_sft_config(config, output_dir, tokenizer, args.max_steps, use_cuda_device)
    trainer = build_trainer(model, tokenizer, data, sft_config, config)
    logger.info("Starting training")
    trainer.train()
    logger.info("Saving adapter to %s", output_dir)
    trainer.save_model(str(output_dir))
    tokenizer.save_pretrained(str(output_dir))
    return 0


if __name__ == "__main__":
    sys.exit(main())
