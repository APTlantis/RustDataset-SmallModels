#!/usr/bin/env python
"""Qualitative prompt evals for Rust fine-tuned adapters."""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any

import torch
from peft import PeftModel
from transformers import AutoModelForCausalLM, AutoTokenizer, set_seed


@dataclass(frozen=True)
class EvalPrompt:
    id: str
    category: str
    system: str
    user: str


PROMPTS: list[EvalPrompt] = [
    EvalPrompt(
        "concept-ownership-move",
        "concept",
        "You explain Rust concepts clearly with small correct examples.",
        "Explain why this Rust code moves `name`, and show one idiomatic fix:\n\n```rust\nfn main() {\n    let name = String::from(\"Ferris\");\n    let other = name;\n    println!(\"{name}\");\n}\n```",
    ),
    EvalPrompt(
        "concept-borrowing-mutability",
        "concept",
        "You explain Rust concepts clearly with small correct examples.",
        "Explain the difference between `&T` and `&mut T` in Rust. Include a tiny example.",
    ),
    EvalPrompt(
        "concept-result-question-mark",
        "concept",
        "You explain Rust concepts clearly with small correct examples.",
        "What does the `?` operator do with `Result` in Rust? Show a compact function that uses it.",
    ),
    EvalPrompt(
        "concept-trait-bound",
        "concept",
        "You explain Rust concepts clearly with small correct examples.",
        "Explain a generic function with a trait bound using `Display`.",
    ),
    EvalPrompt(
        "concept-iterator-map-filter",
        "concept",
        "You explain Rust concepts clearly with small correct examples.",
        "Explain how `iter`, `filter`, and `map` can be chained in Rust. Include a short example.",
    ),
    EvalPrompt(
        "completion-sum-even",
        "completion",
        "You complete Rust code accurately and idiomatically.",
        "Complete this Rust function:\n\n```rust\npub fn sum_even(values: &[i32]) -> i32 {\n    values\n        .iter()\n        // TODO: complete the iterator chain\n```",
    ),
    EvalPrompt(
        "completion-parse-port",
        "completion",
        "You complete Rust code accurately and idiomatically.",
        "Complete this Rust function:\n\n```rust\npub fn parse_port(raw: &str) -> Result<u16, std::num::ParseIntError> {\n    let port = raw.trim()\n        // TODO: parse the port\n```",
    ),
    EvalPrompt(
        "completion-count-words",
        "completion",
        "You complete Rust code accurately and idiomatically.",
        "Complete this Rust function:\n\n```rust\nuse std::collections::HashMap;\n\npub fn count_words(text: &str) -> HashMap<String, usize> {\n    let mut counts = HashMap::new();\n    // TODO: fill the map\n```",
    ),
    EvalPrompt(
        "completion-first-long",
        "completion",
        "You complete Rust code accurately and idiomatically.",
        "Complete this Rust function:\n\n```rust\npub fn first_long_word(words: &[String], min_len: usize) -> Option<&str> {\n    words\n        .iter()\n        // TODO: return the first matching word as &str\n```",
    ),
    EvalPrompt(
        "completion-join-paths",
        "completion",
        "You complete Rust code accurately and idiomatically.",
        "Complete this Rust function:\n\n```rust\nuse std::path::PathBuf;\n\npub fn config_path(root: &str, file: &str) -> PathBuf {\n    let mut path = PathBuf::from(root);\n    // TODO: append file and return path\n```",
    ),
    EvalPrompt(
        "repair-missing-brace",
        "repair",
        "You fix Rust code errors and explain the correction briefly.",
        "Fix this Rust code:\n\n```rust\npub fn clamp_to_zero(value: i32) -> i32 {\n    if value < 0 {\n        0\n    } else {\n        value\n    }\n```",
    ),
    EvalPrompt(
        "repair-moved-string",
        "repair",
        "You fix Rust code errors and explain the correction briefly.",
        "Fix this Rust code:\n\n```rust\nfn shout(input: String) {\n    let upper = input.to_uppercase();\n    println!(\"{input}: {upper}\");\n}\n```",
    ),
    EvalPrompt(
        "repair-parse-result",
        "repair",
        "You fix Rust code errors and explain the correction briefly.",
        "Fix this Rust code:\n\n```rust\npub fn parse_count(raw: &str) -> Result<u32, std::num::ParseIntError> {\n    let count: u32 = raw.parse();\n    Ok(count)\n}\n```",
    ),
    EvalPrompt(
        "repair-mut-borrow",
        "repair",
        "You fix Rust code errors and explain the correction briefly.",
        "Fix this Rust code:\n\n```rust\nfn push_name(names: &Vec<String>, name: String) {\n    names.push(name);\n}\n```",
    ),
    EvalPrompt(
        "repair-iterator-type",
        "repair",
        "You fix Rust code errors and explain the correction briefly.",
        "Fix this Rust code:\n\n```rust\npub fn double_all(values: &[i32]) -> Vec<i32> {\n    values.iter().map(|value| value * 2).collect::<i32>()\n}\n```",
    ),
    EvalPrompt(
        "api-option-map",
        "api",
        "You answer Rust API questions accurately and concisely.",
        "When should I use `Option::map` instead of `match`? Include a short Rust example.",
    ),
    EvalPrompt(
        "api-vec-retain",
        "api",
        "You answer Rust API questions accurately and concisely.",
        "How does `Vec::retain` work, and when is it useful?",
    ),
    EvalPrompt(
        "api-string-as-str",
        "api",
        "You answer Rust API questions accurately and concisely.",
        "What is the difference between `String`, `&String`, and `&str` for function parameters?",
    ),
]


def load_config(path: Path | None) -> dict[str, Any]:
    if path is None or not path.exists():
        return {}
    with path.open("rb") as handle:
        return tomllib.load(handle)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run qualitative Rust prompt evals.")
    parser.add_argument("--config", type=Path, default=Path("training/rust_cpu_layer2.toml"))
    parser.add_argument("--model-name", type=str)
    parser.add_argument("--adapter", type=Path, action="append", default=[])
    parser.add_argument("--include-base", action="store_true")
    parser.add_argument("--output", type=Path, default=Path("training/evals/rust-layer2-balanced-80.json"))
    parser.add_argument("--markdown", type=Path)
    parser.add_argument("--max-new-tokens", type=int, default=192)
    parser.add_argument("--temperature", type=float, default=0.0)
    parser.add_argument("--limit", type=int, help="Run only the first N prompts.")
    parser.add_argument("--seed", type=int)
    parser.add_argument("--cpu", action="store_true", help="Force CPU.")
    parser.add_argument("--dry-run", action="store_true", help="Write prompt metadata without loading a model.")
    return parser.parse_args()


def prompt_messages(prompt: EvalPrompt) -> list[dict[str, str]]:
    return [
        {"role": "system", "content": prompt.system},
        {"role": "user", "content": prompt.user},
    ]


def render_prompt(tokenizer, prompt: EvalPrompt) -> str:
    messages = prompt_messages(prompt)
    if getattr(tokenizer, "chat_template", None):
        return tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
    return "\n".join(f"<|{message['role']}|>\n{message['content']}" for message in messages) + "\n<|assistant|>\n"


def load_tokenizer(model_name: str):
    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    return tokenizer


def load_base_model(model_name: str, force_cpu: bool):
    use_cuda = torch.cuda.is_available() and not force_cpu
    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        device_map="auto" if use_cuda else None,
        torch_dtype=torch.float16 if use_cuda else torch.float32,
        trust_remote_code=True,
        low_cpu_mem_usage=True,
    )
    model.eval()
    return model


def generate_one(model, tokenizer, prompt: EvalPrompt, args: argparse.Namespace) -> str:
    rendered = render_prompt(tokenizer, prompt)
    inputs = tokenizer(rendered, return_tensors="pt")
    inputs = {key: value.to(model.device) for key, value in inputs.items()}
    do_sample = args.temperature > 0.0
    generation_kwargs = {
        "max_new_tokens": args.max_new_tokens,
        "do_sample": do_sample,
        "pad_token_id": tokenizer.eos_token_id,
        "eos_token_id": tokenizer.eos_token_id,
    }
    if do_sample:
        generation_kwargs["temperature"] = args.temperature
    with torch.no_grad():
        output = model.generate(**inputs, **generation_kwargs)
    generated = output[0][inputs["input_ids"].shape[-1] :]
    return tokenizer.decode(generated, skip_special_tokens=True).strip()


def model_specs(args: argparse.Namespace) -> list[tuple[str, Path | None]]:
    specs: list[tuple[str, Path | None]] = []
    if args.include_base or not args.adapter:
        specs.append(("base", None))
    for adapter in args.adapter:
        specs.append((adapter.name, adapter))
    return specs


def run_eval(args: argparse.Namespace, config: dict[str, Any]) -> dict[str, Any]:
    model_name = args.model_name or config.get("model", {}).get("name", "TinyLlama/TinyLlama-1.1B-Chat-v1.0")
    seed = args.seed if args.seed is not None else int(config.get("training", {}).get("seed", 42))
    set_seed(seed)
    selected_prompts = PROMPTS[: args.limit] if args.limit else PROMPTS
    specs = model_specs(args)

    report: dict[str, Any] = {
        "model_name": model_name,
        "seed": seed,
        "max_new_tokens": args.max_new_tokens,
        "temperature": args.temperature,
        "prompts": [asdict(prompt) for prompt in selected_prompts],
        "models": [],
    }

    if args.dry_run:
        report["models"] = [{"name": name, "adapter": str(adapter) if adapter else None, "responses": []} for name, adapter in specs]
        return report

    tokenizer = load_tokenizer(model_name)
    for name, adapter in specs:
        base_model = load_base_model(model_name, args.cpu)
        model = PeftModel.from_pretrained(base_model, str(adapter)) if adapter else base_model
        model.eval()
        responses = []
        for prompt in selected_prompts:
            responses.append(
                {
                    "prompt_id": prompt.id,
                    "category": prompt.category,
                    "response": generate_one(model, tokenizer, prompt, args),
                }
            )
        report["models"].append({"name": name, "adapter": str(adapter) if adapter else None, "responses": responses})
        del model
        del base_model
    return report


def write_json(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


def write_markdown(path: Path, report: dict[str, Any]) -> None:
    prompt_by_id = {prompt["id"]: prompt for prompt in report["prompts"]}
    lines = [
        "# Rust Qualitative Eval",
        "",
        f"- Model: `{report['model_name']}`",
        f"- Max new tokens: `{report['max_new_tokens']}`",
        f"- Temperature: `{report['temperature']}`",
        "",
    ]
    for model in report["models"]:
        lines.extend([f"## {model['name']}", ""])
        for response in model["responses"]:
            prompt = prompt_by_id[response["prompt_id"]]
            lines.extend(
                [
                    f"### {response['prompt_id']} ({response['category']})",
                    "",
                    "**Prompt**",
                    "",
                    prompt["user"],
                    "",
                    "**Response**",
                    "",
                    response["response"] or "_No response_",
                    "",
                ]
            )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    config = load_config(args.config)
    report = run_eval(args, config)
    markdown = args.markdown or args.output.with_suffix(".md")
    write_json(args.output, report)
    write_markdown(markdown, report)
    print(json.dumps({"output": str(args.output), "markdown": str(markdown), "models": len(report["models"]), "prompts": len(report["prompts"])}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
