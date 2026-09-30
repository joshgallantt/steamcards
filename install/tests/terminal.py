"""Runs a command with a terminal of its own, or with none, for uninstall.sh's tests.

    python3 terminal.py none COMMAND [ARGUMENT...]
    python3 terminal.py answer ANSWER TRANSCRIPT COMMAND [ARGUMENT...]

uninstall.sh asks its question on the terminal, /dev/tty, since what comes in
on stdin is the script itself, piped from curl. With `none`, COMMAND runs
with no terminal at all, as on CI or from cron, so /dev/tty can't be opened.
With `answer`, it runs with a terminal of its own, as when someone pastes the
one-liner into Terminal: what it writes there goes to TRANSCRIPT, and once
it's written something (its question), ANSWER is typed in, then Enter.

Either way, COMMAND's input, output and error output are this program's, so a
script piped in is still what comes in on stdin. It exits as COMMAND does.
Python's standard library only, on macOS or Linux.
"""

import fcntl
import os
import select
import signal
import sys
import termios
import time

# Long enough for any run of uninstall.sh, so that one left waiting for an
# answer fails its scenario instead of holding up the tests.
TIMEOUT = 30


def start(command, terminal=None, controller=None):
    pid = os.fork()
    if pid == 0:
        try:
            # A session of its own has no terminal, until it's given one.
            os.setsid()
            if terminal is not None:
                fcntl.ioctl(terminal, termios.TIOCSCTTY, 0)
                os.close(terminal)
                os.close(controller)
            os.execvp(command[0], command)
        except OSError as error:
            sys.stderr.write("terminal.py: %s: %s\n" % (command[0], error))
        os._exit(127)
    return pid


def finished(pid, deadline):
    """COMMAND's exit code if it's finished, stopping it once it's too late."""
    done, status = os.waitpid(pid, os.WNOHANG)
    if not done and time.monotonic() > deadline:
        sys.stderr.write("terminal.py: still running after %d seconds, so stopped\n" % TIMEOUT)
        os.kill(pid, signal.SIGKILL)
        done, status = os.waitpid(pid, 0)
    if not done:
        return None
    if os.WIFSIGNALED(status):
        return 128 + os.WTERMSIG(status)
    return os.WEXITSTATUS(status)


def without_terminal(command):
    pid = start(command)
    deadline = time.monotonic() + TIMEOUT
    while True:
        code = finished(pid, deadline)
        if code is not None:
            return code
        time.sleep(0.02)


def with_terminal(answer, transcript, command):
    controller, terminal = os.openpty()
    pid = start(command, terminal, controller)
    # This end of the terminal stays open here too, so reading from the
    # controller waits for what COMMAND writes, even between the times it
    # opens /dev/tty.
    deadline = time.monotonic() + TIMEOUT
    shown = b""
    answered = False
    code = None
    while code is None:
        code = finished(pid, deadline)
        # Once it's finished, what it wrote last.
        wait = 0 if code is not None else 0.02
        while select.select([controller], [], [], wait)[0]:
            chunk = os.read(controller, 4096)
            if not chunk:
                break
            shown += chunk
            if not answered:
                os.write(controller, answer.encode() + b"\n")
                answered = True
    os.close(controller)
    os.close(terminal)
    with open(transcript, "wb") as f:
        f.write(shown)
    return code


def main():
    args = sys.argv[1:]
    if len(args) >= 2 and args[0] == "none":
        return without_terminal(args[1:])
    if len(args) >= 4 and args[0] == "answer":
        return with_terminal(args[1], args[2], args[3:])
    sys.exit(__doc__.split("\n\n")[1])


if __name__ == "__main__":
    sys.exit(main())
