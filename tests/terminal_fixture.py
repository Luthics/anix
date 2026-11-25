"""Disposable controlling terminals for CLI conformance checks."""
from contextlib import contextmanager
import fcntl
import os
import pty
import select
import subprocess
import termios
import time


@contextmanager
def interactive_shell(argv, cwd, env):
    """Give job control and keyboard signals to a disposable shell."""
    master, slave = pty.openpty()

    def acquire_terminal():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    try:
        shell = subprocess.Popen(argv, cwd=cwd, env=env, stdin=slave, stdout=slave,
                                 stderr=slave, preexec_fn=acquire_terminal)
        try:
            yield master
        finally:
            # Closing the master also releases a shell exiting on Darwin.
            os.close(master)
            master = -1
            try:
                shell.wait(timeout=2)
            except subprocess.TimeoutExpired:
                shell.kill()
                shell.wait(timeout=2)
    finally:
        os.close(slave)
        if master >= 0:
            os.close(master)


def read_until_any(master, markers):
    seen = bytearray()
    deadline = time.monotonic() + 8
    while True:
        for marker in markers:
            if marker in seen:
                return marker, bytes(seen)
        timeout = deadline - time.monotonic()
        if timeout <= 0 or not select.select([master], [], [], timeout)[0]:
            raise AssertionError(f"timed out waiting for {markers!r}: {bytes(seen)!r}")
        chunk = os.read(master, 8192)
        if not chunk:
            raise AssertionError(f"terminal closed before {markers!r}: {bytes(seen)!r}")
        seen.extend(chunk)


def read_until(master, marker):
    return read_until_any(master, (marker,))[1]
