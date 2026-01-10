#!/usr/bin/env bash
# Test utilities for agent CLI

# Test assertion helpers
assert_eq() {
    local expected="$1"
    local actual="$2"
    local msg="${3:-assertion}"
    if [[ "$expected" != "$actual" ]]; then
        echo "FAIL: $msg"
        echo "  Expected: $expected"
        echo "  Actual: $actual"
        return 1
    fi
    echo "PASS: $msg"
    return 0
}

assert_contains() {
    local haystack="$1"
    local needle="$2"
    local msg="${3:-assertion}"
    if [[ "$haystack" != *"$needle"* ]]; then
        echo "FAIL: $msg"
        echo "  Expected to contain: $needle"
        echo "  Actual: $haystack"
        return 1
    fi
    echo "PASS: $msg"
    return 0
}

assert_not_empty() {
    local value="$1"
    local msg="${2:-value should not be empty}"
    if [[ -z "$value" ]]; then
        echo "FAIL: $msg"
        return 1
    fi
    echo "PASS: $msg"
    return 0
}

assert_file_exists() {
    local path="$1"
    local msg="${2:-file should exist: $path}"
    if [[ ! -f "$path" ]]; then
        echo "FAIL: $msg"
        return 1
    fi
    echo "PASS: $msg"
    return 0
}

assert_command_exists() {
    local cmd="$1"
    local msg="${2:-command should exist: $cmd}"
    if ! command -v "$cmd" &>/dev/null; then
        echo "FAIL: $msg"
        return 1
    fi
    echo "PASS: $msg"
    return 0
}

# Setup test environment
setup_test_env() {
    export TEST_DIR=$(mktemp -d)
    export CONFIG_DIR="$TEST_DIR/config"
    export DATA_DIR="$TEST_DIR/data"
    export TEMPLATES_DIR="$TEST_DIR/config/templates"
    export AGENTS_FILE="$DATA_DIR/agents.json"
    mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$TEMPLATES_DIR"

    # Initialize agents.json
    echo '{"agents":{},"next_slot":1,"max_slots":9}' > "$AGENTS_FILE"
}

# Teardown test environment
teardown_test_env() {
    if [[ -n "${TEST_DIR:-}" ]] && [[ -d "$TEST_DIR" ]]; then
        rm -rf "$TEST_DIR"
    fi
}

# Run a test function with setup/teardown
run_test() {
    local test_name="$1"
    local test_func="$2"

    echo "Running: $test_name"
    setup_test_env

    local result=0
    if $test_func; then
        echo "  OK"
    else
        echo "  FAILED"
        result=1
    fi

    teardown_test_env
    return $result
}
