"""
Notification dispatch for loop events.

Sends notifications when loops complete, fail, or need attention.
Supports multiple backends for different environments.
"""

import subprocess
import os
from typing import Optional
from enum import Enum


class NotifyLevel(Enum):
    """Notification priority level."""
    INFO = "info"
    SUCCESS = "success"
    WARNING = "warning"
    ERROR = "error"


def notify(
    title: str,
    message: str,
    level: NotifyLevel = NotifyLevel.INFO,
    sound: bool = False
) -> bool:
    """
    Send a notification using the best available method.

    Tries in order:
    1. notify-send (Linux)
    2. osascript (macOS)
    3. terminal-notifier (macOS, if installed)
    4. PowerShell (Windows)
    5. Print to stdout (fallback)

    Args:
        title: Notification title
        message: Notification body
        level: Priority level
        sound: Play sound with notification

    Returns:
        True if notification was sent successfully
    """
    # Try notify-send (Linux)
    if _try_notify_send(title, message, level):
        return True

    # Try osascript (macOS)
    if _try_osascript(title, message, sound):
        return True

    # Try terminal-notifier (macOS)
    if _try_terminal_notifier(title, message, level, sound):
        return True

    # Try PowerShell (Windows)
    if _try_powershell(title, message):
        return True

    # Fallback: print to stdout
    print(f"\n[{level.value.upper()}] {title}: {message}\n")
    return True


def _try_notify_send(title: str, message: str, level: NotifyLevel) -> bool:
    """Try Linux notify-send."""
    try:
        urgency_map = {
            NotifyLevel.INFO: "low",
            NotifyLevel.SUCCESS: "normal",
            NotifyLevel.WARNING: "normal",
            NotifyLevel.ERROR: "critical"
        }
        urgency = urgency_map.get(level, "normal")

        result = subprocess.run(
            ["notify-send", "-u", urgency, title, message],
            capture_output=True,
            timeout=5
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def _try_osascript(title: str, message: str, sound: bool) -> bool:
    """Try macOS osascript."""
    try:
        # Escape quotes for AppleScript
        title = title.replace('"', '\\"')
        message = message.replace('"', '\\"')

        script = f'display notification "{message}" with title "{title}"'
        if sound:
            script += ' sound name "default"'

        result = subprocess.run(
            ["osascript", "-e", script],
            capture_output=True,
            timeout=5
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def _try_terminal_notifier(
    title: str,
    message: str,
    level: NotifyLevel,
    sound: bool
) -> bool:
    """Try macOS terminal-notifier."""
    try:
        cmd = ["terminal-notifier", "-title", title, "-message", message]

        if sound:
            cmd.extend(["-sound", "default"])

        # Set group for deduplication
        cmd.extend(["-group", "daedalos-loop"])

        result = subprocess.run(cmd, capture_output=True, timeout=5)
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def _try_powershell(title: str, message: str) -> bool:
    """Try Windows PowerShell notification."""
    try:
        # Escape for PowerShell
        title = title.replace("'", "''")
        message = message.replace("'", "''")

        script = f"""
        [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
        $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
        $textNodes = $template.GetElementsByTagName("text")
        $textNodes.Item(0).AppendChild($template.CreateTextNode('{title}')) | Out-Null
        $textNodes.Item(1).AppendChild($template.CreateTextNode('{message}')) | Out-Null
        $toast = [Windows.UI.Notifications.ToastNotification]::new($template)
        [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Daedalos').Show($toast)
        """

        result = subprocess.run(
            ["powershell", "-Command", script],
            capture_output=True,
            timeout=10
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def notify_loop_complete(
    loop_id: str,
    prompt: str,
    iterations: int,
    success: bool
):
    """Send notification for loop completion."""
    if success:
        title = "Loop Complete"
        message = f"'{prompt[:50]}...' succeeded in {iterations} iterations"
        level = NotifyLevel.SUCCESS
    else:
        title = "Loop Failed"
        message = f"'{prompt[:50]}...' failed after {iterations} iterations"
        level = NotifyLevel.ERROR

    notify(title, message, level, sound=True)


def notify_loop_needs_attention(loop_id: str, reason: str):
    """Send notification when loop needs human attention."""
    notify(
        "Loop Needs Attention",
        f"Loop {loop_id}: {reason}",
        NotifyLevel.WARNING,
        sound=True
    )


def notify_workflow_complete(workflow_name: str, success: bool, loops_run: int):
    """Send notification for workflow completion."""
    if success:
        title = "Workflow Complete"
        message = f"'{workflow_name}' completed ({loops_run} loops)"
        level = NotifyLevel.SUCCESS
    else:
        title = "Workflow Failed"
        message = f"'{workflow_name}' failed"
        level = NotifyLevel.ERROR

    notify(title, message, level, sound=True)


# Custom notification command support
_custom_notify_cmd: Optional[str] = None


def set_custom_notify_command(command: str):
    """
    Set a custom notification command.

    The command will be called with:
    - $TITLE - notification title
    - $MESSAGE - notification message
    - $LEVEL - notification level

    Example:
        set_custom_notify_command("my-notifier --title '$TITLE' --msg '$MESSAGE'")
    """
    global _custom_notify_cmd
    _custom_notify_cmd = command


def custom_notify(title: str, message: str, level: NotifyLevel) -> bool:
    """Send notification using custom command if set."""
    global _custom_notify_cmd
    if not _custom_notify_cmd:
        return False

    try:
        env = os.environ.copy()
        env["TITLE"] = title
        env["MESSAGE"] = message
        env["LEVEL"] = level.value

        result = subprocess.run(
            _custom_notify_cmd,
            shell=True,
            env=env,
            capture_output=True,
            timeout=10
        )
        return result.returncode == 0
    except (subprocess.TimeoutExpired, Exception):
        return False
