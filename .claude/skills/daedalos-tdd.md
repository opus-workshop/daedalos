---
name: daedalos-tdd
description: Use when implementing ANY feature - write tests FIRST, then implement. Tests are the specification.
---

# Daedalos TDD - Test-Driven Development

## Overview

Tests are the specification. Write them BEFORE code. Implementation is "just" making the tests pass.

## MANDATORY WORKFLOW

```dot
digraph tdd {
    rankdir=LR;
    "RED" [style=filled, fillcolor=red, fontcolor=white];
    "GREEN" [style=filled, fillcolor=green];
    "REFACTOR" [style=filled, fillcolor=blue, fontcolor=white];

    "RED" -> "GREEN" -> "REFACTOR" -> "RED";
}
```

1. **RED**: Write a failing test (defines what you want)
2. **GREEN**: Write minimal code to pass the test
3. **REFACTOR**: Clean up while keeping tests green

## When to Use

- Adding ANY new feature
- Fixing bugs (write test that reproduces, then fix)
- Before refactoring (ensure baseline coverage first)

## Step-by-Step

### Phase 1: RED - Write Failing Test

```bash
# Create checkpoint
undo checkpoint "before-tdd-feature"

# Write the test FIRST - it should FAIL
# This test defines what you want the code to do
```

Example test (write this BEFORE implementation):

```python
def test_user_can_login_with_valid_credentials():
    user = create_user(email="test@example.com", password="secure123")
    result = login(email="test@example.com", password="secure123")
    assert result.success == True
    assert result.user.email == "test@example.com"
```

Verify it fails:

```bash
verify --quick  # Should show test failure
```

### Phase 2: GREEN - Make It Pass

Write the MINIMUM code to make the test pass:

```bash
# Start a loop to iterate until tests pass
loop start "make login test pass" --promise "pytest tests/test_auth.py -v"
```

Rules for GREEN phase:
- Write ONLY enough code to pass the test
- Don't optimize yet
- Don't add features the test doesn't require
- "Fake it till you make it" is okay

### Phase 3: REFACTOR - Clean Up

Once tests pass, refactor:

```bash
# Tests are your safety net - refactor freely
loop start "refactor auth code" --promise "verify"
```

Rules for REFACTOR phase:
- Tests must stay green
- Improve code quality, readability, performance
- Extract functions, rename variables, remove duplication
- Run `verify` frequently

## Using the TDD Template

For the full workflow with multi-agent support:

```bash
loop start --template tdd "add password reset feature"
```

This spawns:
1. **Planner** - Designs test cases (read-only)
2. **Tester** - Writes failing tests
3. **Implementer** - Makes tests pass
4. **Verifier** - Confirms coverage and quality

## Test Quality Checklist

Before moving from RED to GREEN:

- [ ] Test has a clear, descriptive name
- [ ] Test checks ONE thing
- [ ] Test would fail without the feature
- [ ] Test documents expected behavior
- [ ] Edge cases are covered

## Red Flags - STOP These Patterns

| Thought | Reality |
|---------|---------|
| "I'll write tests after" | Tests written after are weaker. They test implementation, not behavior. |
| "This is too simple to test" | Simple code grows complex. Test it now. |
| "I know what I need to build" | The test IS the specification. Write it first. |
| "Tests slow me down" | Tests speed you up. Debugging without tests is slower. |
| "I'll just run it manually" | Manual testing doesn't scale or regress-protect. |

## Example: Adding Email Validation

### Step 1: RED

```python
# tests/test_validation.py
def test_valid_email_accepted():
    assert validate_email("user@example.com") == True

def test_invalid_email_rejected():
    assert validate_email("not-an-email") == False

def test_empty_email_rejected():
    assert validate_email("") == False

def test_email_without_domain_rejected():
    assert validate_email("user@") == False
```

Run tests - they should fail (function doesn't exist):

```bash
pytest tests/test_validation.py -v
# FAILED - NameError: validate_email is not defined
```

### Step 2: GREEN

```bash
loop start "implement validate_email to pass tests" --promise "pytest tests/test_validation.py"
```

Write minimal implementation:

```python
# src/validation.py
import re

def validate_email(email: str) -> bool:
    if not email:
        return False
    pattern = r'^[^@]+@[^@]+\.[^@]+$'
    return bool(re.match(pattern, email))
```

### Step 3: REFACTOR

Tests pass. Now improve:

```bash
loop start "refactor validate_email" --promise "verify"
```

Maybe extract pattern, add type hints, improve readability.

## Integration

```bash
# Full TDD session
undo checkpoint "before-feature"
# Write tests...
verify --quick  # Confirm tests fail
loop start "implement feature" --promise "pytest"
verify  # Full check after implementation
```

## Philosophy

"Tests are not a burden. Tests are a gift you give your future self."

TDD isn't about testing. It's about:
- Thinking before coding
- Designing from the outside in
- Building confidence through verification
- Creating living documentation
