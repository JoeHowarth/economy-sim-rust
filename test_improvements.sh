#!/bin/bash
# Test script to demonstrate the new improvements

echo "=== Testing Village Model Improvements ==="
echo

# Build the project
echo "Building project..."
cargo build --release

# Show help
echo "1. Enhanced help with new commands:"
./target/release/village-model-sim --help | head -20
echo

# Test quiet mode
echo "2. Testing quiet mode (no output except events saved message):"
./target/release/village-model-sim run --scenario-file scenarios/trading_specialization.json -s balanced -s balanced -s balanced --quiet -o results/quiet_test.json
echo

# Test query functionality
echo "3. Testing query functionality:"
echo "   - Query all trade events for food_specialist:"
./target/release/village-model-sim query results/quiet_test.json --village food_specialist --event-type trade
echo

# Test analyze-batch
echo "4. Testing analyze-batch on existing results:"
./target/release/village-model-sim analyze-batch strategy_evaluation/*.json -o results/batch_analysis.csv
echo

# Show CSV output
echo "5. CSV output preview:"
head -5 results/batch_analysis.csv
echo

echo "=== Test Complete ==="