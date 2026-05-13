"""Clock abstractions for monotonic time."""

from __future__ import annotations

import threading
import time
from typing import Protocol


class Timestamp:
    """Monotonic timestamp in nanoseconds since engine creation."""

    __slots__ = ("_nanos",)

    ZERO: Timestamp

    def __init__(self, nanos: int) -> None:
        self._nanos = nanos

    @property
    def as_nanos(self) -> int:
        return self._nanos

    @property
    def as_millis(self) -> int:
        return self._nanos // 1_000_000

    @property
    def as_secs_f64(self) -> float:
        return self._nanos / 1_000_000_000.0

    def duration_since(self, other: Timestamp) -> int:
        """Duration since another timestamp in nanoseconds. Returns 0 if other is after self."""
        return max(0, self._nanos - other._nanos)

    def add_nanos(self, nanos: int) -> Timestamp:
        return Timestamp(self._nanos + nanos)

    def add_millis(self, millis: int) -> Timestamp:
        return Timestamp(self._nanos + millis * 1_000_000)

    def add_secs(self, secs: int) -> Timestamp:
        return Timestamp(self._nanos + secs * 1_000_000_000)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Timestamp):
            return NotImplemented
        return self._nanos == other._nanos

    def __lt__(self, other: Timestamp) -> bool:
        return self._nanos < other._nanos

    def __le__(self, other: Timestamp) -> bool:
        return self._nanos <= other._nanos

    def __gt__(self, other: Timestamp) -> bool:
        return self._nanos > other._nanos

    def __ge__(self, other: Timestamp) -> bool:
        return self._nanos >= other._nanos

    def __repr__(self) -> str:
        return f"Timestamp({self._nanos})"


Timestamp.ZERO = Timestamp(0)


class Clock(Protocol):
    """Clock abstraction for monotonic time."""

    def now(self) -> Timestamp: ...


class RealClock:
    """Real monotonic clock backed by time.monotonic_ns()."""

    def __init__(self) -> None:
        self._epoch = time.monotonic_ns()

    def now(self) -> Timestamp:
        return Timestamp(time.monotonic_ns() - self._epoch)


class MockClock:
    """Mock clock for deterministic testing. Time only advances when explicitly told to."""

    def __init__(self) -> None:
        self._nanos = 0
        self._lock = threading.Lock()

    @classmethod
    def at(cls, timestamp: Timestamp) -> MockClock:
        clock = cls()
        clock._nanos = timestamp.as_nanos
        return clock

    def now(self) -> Timestamp:
        with self._lock:
            return Timestamp(self._nanos)

    def advance_nanos(self, nanos: int) -> None:
        with self._lock:
            self._nanos += nanos

    def advance_ms(self, ms: int) -> None:
        self.advance_nanos(ms * 1_000_000)

    def advance_secs(self, secs: int) -> None:
        self.advance_nanos(secs * 1_000_000_000)

    def set(self, timestamp: Timestamp) -> None:
        with self._lock:
            self._nanos = timestamp.as_nanos
