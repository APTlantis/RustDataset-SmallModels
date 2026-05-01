param(
    [string]$ExternalDir = "external_sources",
    [string]$Out = "out_layer2",
    [string]$Work = "work_layer2",
    [string]$PreparedDir = "training/data/rust-layer2",
    [int]$MaxExamples = 1024,
    [switch]$SkipFetch,
    [switch]$ValidateCode
)

$ErrorActionPreference = "Stop"

function Sync-Repo {
    param(
        [string]$Url,
        [string]$Path
    )

    if ($SkipFetch) {
        return
    }

    if (Test-Path (Join-Path $Path ".git")) {
        git -C $Path pull --ff-only
    } else {
        git clone --depth 1 $Url $Path
    }
}

New-Item -ItemType Directory -Force -Path $ExternalDir, $Out, $Work, $PreparedDir | Out-Null

$book = Join-Path $ExternalDir "book"
$rbe = Join-Path $ExternalDir "rust-by-example"
$rustlings = Join-Path $ExternalDir "rustlings"

Sync-Repo "https://github.com/rust-lang/book.git" $book
Sync-Repo "https://github.com/rust-lang/rust-by-example.git" $rbe
Sync-Repo "https://github.com/rust-lang/rustlings.git" $rustlings

cargo run -- clean --out $Out --work $Work
cargo run -- generate-samples --output $Out

cargo run -- ingest-mdbook --input (Join-Path $book "src") --output (Join-Path $Work "book_chunks.jsonl")
cargo run -- ingest-mdbook --input (Join-Path $rbe "src") --output (Join-Path $Work "rust_by_example_chunks.jsonl")
python training/combine_intermediates.py --kind mdbook --output (Join-Path $Work "layer2_book_chunks.jsonl") (Join-Path $Work "book_chunks.jsonl") (Join-Path $Work "rust_by_example_chunks.jsonl")
cargo run -- generate-sft --input (Join-Path $Work "layer2_book_chunks.jsonl") --output (Join-Path $Out "rust_concepts_sft.jsonl")

cargo run -- ingest-crates --input . --output (Join-Path $Work "project_code_items.jsonl")
cargo run -- ingest-crates --input $rustlings --output (Join-Path $Work "rustlings_code_items.jsonl")
python training/combine_intermediates.py --kind code --output (Join-Path $Work "layer2_code_items.jsonl") (Join-Path $Work "project_code_items.jsonl") (Join-Path $Work "rustlings_code_items.jsonl")
cargo run -- generate-completion --input (Join-Path $Work "layer2_code_items.jsonl") --output (Join-Path $Out "rust_code_completion.jsonl")
cargo run -- generate-repair --input (Join-Path $Work "layer2_code_items.jsonl") --output (Join-Path $Out "rust_code_repair.jsonl")

if ($ValidateCode) {
    cargo run -- validate-code --input (Join-Path $Out "rust_code_completion.jsonl") --output (Join-Path $Out "rust_code_completion.jsonl") --work-dir (Join-Path $Work "cargo_validate_completion")
    cargo run -- validate-code --input (Join-Path $Out "rust_code_repair.jsonl") --output (Join-Path $Out "rust_code_repair.jsonl") --work-dir (Join-Path $Work "cargo_validate_repair")
}

cargo run -- validate --input $Out --report (Join-Path $Out "quality_report.json")
cargo run -- export-parquet --input $Out --output (Join-Path $Out "rust_corpus.parquet")
cargo run -- manifest --input $Out --output (Join-Path $Out "corpus_manifest.toml")
cargo run -- hashes --input $Out --output (Join-Path $Out "snapshot-hashes.txt")
python training/prepare_rust_train.py --config training/rust_cpu_layer2.toml --input-dir $Out --output-dir $PreparedDir --max-examples $MaxExamples

Write-Host "Layer 2 dataset ready:"
Write-Host "  corpus:   $Out"
Write-Host "  work:     $Work"
Write-Host "  prepared: $PreparedDir"
