#!/usr/bin/env bash

echo "Launching Agent"
copilot --allow-tool "shell(bn:*)" --allow-tool "shell(./target/release/bn)" --allow-tool "shell(cargo fmt)" --allow-tool "shell(cargo clippy)" --allow-tool "shell(cargo test)" --allow-tool "shell(cargo build)" --allow-tool "shell(sleep)" --allow-tool "shell(wait)" --allow-tool "shell(git add)" --allow-tool "shell(git commit)"
