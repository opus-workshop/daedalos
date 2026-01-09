#!/usr/bin/env bash
# Tests for agent CRUD operations

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/test_common.sh"
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/agents.sh"

# Test agent initialization
test_agents_init() {
    setup_test_env
    agents_init
    assert_file_exists "$AGENTS_FILE" "agents.json should be created"
    teardown_test_env
}

# Test agent creation
test_agents_create() {
    setup_test_env
    agents_init

    # Mock tmux_session_name function
    tmux_session_name() { echo "claude-agent-$1"; }

    agents_create "test-agent" "/tmp/project" "implementer" "implement" 1

    local agent
    agent=$(agents_get "test-agent")
    assert_not_empty "$agent" "agent should be created"

    local name
    name=$(echo "$agent" | jq -r '.name')
    assert_eq "test-agent" "$name" "agent name should match"

    teardown_test_env
}

# Test agent exists check
test_agents_exists() {
    setup_test_env
    agents_init

    tmux_session_name() { echo "claude-agent-$1"; }
    agents_create "existing" "/tmp" "default" "implement" 1

    if agents_exists "existing"; then
        echo "PASS: agents_exists returns true for existing agent"
    else
        echo "FAIL: agents_exists should return true"
        teardown_test_env
        return 1
    fi

    if ! agents_exists "nonexistent"; then
        echo "PASS: agents_exists returns false for nonexistent agent"
    else
        echo "FAIL: agents_exists should return false"
        teardown_test_env
        return 1
    fi

    teardown_test_env
}

# Test next slot allocation
test_agents_next_slot() {
    setup_test_env
    agents_init

    local slot
    slot=$(agents_next_slot)
    assert_eq "1" "$slot" "first slot should be 1"

    teardown_test_env
}

# Test agent deletion
test_agents_delete() {
    setup_test_env
    agents_init

    tmux_session_name() { echo "claude-agent-$1"; }
    agents_create "to-delete" "/tmp" "default" "implement" 1

    agents_delete "to-delete"

    if ! agents_exists "to-delete"; then
        echo "PASS: agent deleted successfully"
    else
        echo "FAIL: agent should be deleted"
        teardown_test_env
        return 1
    fi

    teardown_test_env
}

# Test fuzzy matching
test_agents_fuzzy_match() {
    setup_test_env
    agents_init

    tmux_session_name() { echo "claude-agent-$1"; }
    agents_create "auth-implementation" "/tmp" "default" "implement" 1
    agents_create "database-work" "/tmp" "default" "implement" 2

    # Mock has_fzf to return false
    has_fzf() { return 1; }

    local match
    match=$(agents_fuzzy_match "auth")
    assert_eq "auth-implementation" "$match" "should match prefix"

    match=$(agents_fuzzy_match "database")
    assert_eq "database-work" "$match" "should match prefix"

    teardown_test_env
}

# Run all tests
main() {
    local failed=0

    test_agents_init || ((failed++))
    test_agents_create || ((failed++))
    test_agents_exists || ((failed++))
    test_agents_next_slot || ((failed++))
    test_agents_delete || ((failed++))
    test_agents_fuzzy_match || ((failed++))

    echo ""
    echo "========================"
    if [[ $failed -eq 0 ]]; then
        echo "All agent tests passed!"
    else
        echo "$failed test(s) failed"
    fi

    return $failed
}

main
