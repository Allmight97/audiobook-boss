#!/bin/bash

echo "Testing metadata preservation fix..."
echo "=================================="

# Navigate to src-tauri directory and run tests
cd src-tauri || {
    echo "Error: Could not change to src-tauri directory"
    exit 1
}

echo "Running cargo test..."
cargo test --lib audio::processor::tests

echo ""
echo "Running cargo clippy..."
cargo clippy -- -D warnings

echo ""
echo "Testing completed!"