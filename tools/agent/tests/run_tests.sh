#!/usr/bin/env bash
# Run all agent CLI tests

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "========================================"
echo "Agent CLI Test Suite"
echo "========================================"
echo ""

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Run each test file
for test_file in "$SCRIPT_DIR"/test_*.sh; do
    [[ "$(basename "$test_file")" == "test_common.sh" ]] && continue

    echo "Running: $(basename "$test_file")"
    echo "----------------------------------------"

    if bash "$test_file"; then
        ((TESTS_PASSED++))
    else
        ((TESTS_FAILED++))
    fi
    ((TESTS_RUN++))

    echo ""
done

# Summary
echo "========================================"
echo "Summary"
echo "========================================"
echo "Tests Run:    $TESTS_RUN"
echo "Tests Passed: $TESTS_PASSED"
echo "Tests Failed: $TESTS_FAILED"
echo ""

if [[ $TESTS_FAILED -eq 0 ]]; then
    echo "ALL TESTS PASSED"
    exit 0
else
    echo "SOME TESTS FAILED"
    exit 1
fi
