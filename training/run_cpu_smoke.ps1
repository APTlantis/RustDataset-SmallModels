$ErrorActionPreference = "Stop"
$env:PYTHONUTF8 = "1"

python training/prepare_rust_train.py --config training/rust_cpu_smoke.toml
python training/finetune_rust.py --config training/rust_cpu_smoke.toml --dry-run

Write-Host "Dry-run complete. To run a tiny CPU fine-tune:"
Write-Host "python training/finetune_rust.py --config training/rust_cpu_smoke.toml --max-steps 5"
