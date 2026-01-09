"""Tests for the promise verification module."""

import pytest
from pathlib import Path
from lib.promise import verify_promise


class TestVerifyPromise:
    """Test cases for verify_promise function."""

    def test_successful_promise(self, tmp_path: Path):
        """Test that a command exiting with code 0 returns True."""
        result = verify_promise("exit 0", tmp_path)
        assert result is True

    def test_failing_promise(self, tmp_path: Path):
        """Test that a command exiting with non-zero code returns False."""
        result = verify_promise("exit 1", tmp_path)
        assert result is False

    def test_timeout(self, tmp_path: Path):
        """Test that a command exceeding timeout returns False."""
        # sleep for 5 seconds but timeout after 1 second
        result = verify_promise("sleep 5", tmp_path, timeout=1)
        assert result is False
