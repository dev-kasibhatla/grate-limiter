"""Quota tracking strategies: TokenBucket, SlidingWindowCounter, FixedWindow, ConcurrencyLimiter."""

from __future__ import annotations

import math
import threading
from typing import Protocol

from grate_limiter.clock import Timestamp
from grate_limiter.types import Dimension, Window


class QuotaTracker(Protocol):
    """Internal protocol for quota tracking strategies."""

    def check(self, amount: int, now: Timestamp) -> bool: ...
    def record(self, amount: int, now: Timestamp) -> None: ...
    def remaining(self, now: Timestamp) -> int: ...
    def capacity(self) -> int: ...
    def usage_ratio(self, now: Timestamp) -> float: ...
    def burn_rate(self, now: Timestamp) -> float: ...
    def predicted_exhaustion_secs(self, now: Timestamp) -> float: ...
    def reset(self, now: Timestamp) -> None: ...


class TokenBucket:
    """Token bucket quota strategy with continuous refill."""

    def __init__(self, cap: int, window: Window, now: Timestamp) -> None:
        self._capacity = cap
        self._window_nanos = window.as_nanos()
        self._tokens = float(cap)
        self._last_refill = now.as_nanos
        self._consumed_in_window = 0
        self._window_start = now.as_nanos
        self._lock = threading.Lock()

    def _refill(self, now: Timestamp) -> float:
        elapsed = now.as_nanos - self._last_refill
        if elapsed <= 0:
            return self._tokens

        tokens_to_add = (elapsed * self._capacity) / self._window_nanos
        if tokens_to_add <= 0:
            return self._tokens

        self._last_refill = now.as_nanos
        self._tokens = min(self._tokens + tokens_to_add, float(self._capacity))

        # Reset burn rate window if a full window has passed
        if now.as_nanos - self._window_start >= self._window_nanos:
            self._consumed_in_window = 0
            self._window_start = now.as_nanos

        return self._tokens

    def check(self, amount: int, now: Timestamp) -> bool:
        with self._lock:
            available = self._refill(now)
            return available >= amount

    def record(self, amount: int, now: Timestamp) -> None:
        with self._lock:
            self._refill(now)
            self._tokens = max(0.0, self._tokens - amount)
            self._consumed_in_window += amount

    def remaining(self, now: Timestamp) -> int:
        with self._lock:
            return int(self._refill(now))

    def capacity(self) -> int:
        return self._capacity

    def usage_ratio(self, now: Timestamp) -> float:
        cap = self._capacity
        if cap == 0:
            return 1.0
        rem = self.remaining(now)
        return 1.0 - (rem / cap)

    def burn_rate(self, now: Timestamp) -> float:
        with self._lock:
            elapsed_secs = (now.as_nanos - self._window_start) / 1_000_000_000.0
            if elapsed_secs < 0.001:
                return 0.0
            return self._consumed_in_window / elapsed_secs

    def predicted_exhaustion_secs(self, now: Timestamp) -> float:
        rate = self.burn_rate(now)
        if rate <= 0.0:
            return math.inf
        rem = self.remaining(now)
        return rem / rate

    def reset(self, now: Timestamp) -> None:
        with self._lock:
            self._tokens = float(self._capacity)
            self._last_refill = now.as_nanos
            self._consumed_in_window = 0
            self._window_start = now.as_nanos


class SlidingWindowCounter:
    """Sliding window counter quota strategy."""

    def __init__(self, cap: int, window: Window, now: Timestamp) -> None:
        self._capacity = cap
        self._window_nanos = window.as_nanos()
        self._current_count = 0
        self._previous_count = 0
        self._window_start = now.as_nanos
        self._lock = threading.Lock()

    def _rotate_and_count(self, now: Timestamp) -> int:
        elapsed = now.as_nanos - self._window_start

        if elapsed >= 2 * self._window_nanos:
            self._previous_count = 0
            self._current_count = 0
            self._window_start = now.as_nanos
            return 0

        if elapsed >= self._window_nanos:
            current = self._current_count
            self._previous_count = current
            self._current_count = 0
            new_start = self._window_start + self._window_nanos
            self._window_start = new_start

            new_elapsed = now.as_nanos - new_start
            fraction_of_prev = 1.0 - (new_elapsed / self._window_nanos)
            weighted_prev = int(current * fraction_of_prev)
            return weighted_prev

        prev = self._previous_count
        curr = self._current_count
        fraction_of_prev = 1.0 - (elapsed / self._window_nanos)
        weighted_prev = int(prev * fraction_of_prev)
        return weighted_prev + curr

    def check(self, amount: int, now: Timestamp) -> bool:
        with self._lock:
            current_usage = self._rotate_and_count(now)
            return current_usage + amount <= self._capacity

    def record(self, amount: int, now: Timestamp) -> None:
        with self._lock:
            self._rotate_and_count(now)
            self._current_count += amount

    def remaining(self, now: Timestamp) -> int:
        with self._lock:
            used = self._rotate_and_count(now)
            return max(0, self._capacity - used)

    def capacity(self) -> int:
        return self._capacity

    def usage_ratio(self, now: Timestamp) -> float:
        cap = self._capacity
        if cap == 0:
            return 1.0
        rem = self.remaining(now)
        return 1.0 - (rem / cap)

    def burn_rate(self, now: Timestamp) -> float:
        with self._lock:
            elapsed_secs = (now.as_nanos - self._window_start) / 1_000_000_000.0
            if elapsed_secs < 0.001:
                return 0.0
            return self._current_count / elapsed_secs

    def predicted_exhaustion_secs(self, now: Timestamp) -> float:
        rate = self.burn_rate(now)
        if rate <= 0.0:
            return math.inf
        rem = self.remaining(now)
        return rem / rate

    def reset(self, now: Timestamp) -> None:
        with self._lock:
            self._current_count = 0
            self._previous_count = 0
            self._window_start = now.as_nanos


class FixedWindow:
    """Fixed window quota strategy."""

    def __init__(self, cap: int, window: Window, now: Timestamp) -> None:
        self._capacity = cap
        self._window_nanos = window.as_nanos()
        self._count = 0
        self._window_start = now.as_nanos
        self._lock = threading.Lock()

    def _maybe_reset(self, now: Timestamp) -> None:
        elapsed = now.as_nanos - self._window_start
        if elapsed >= self._window_nanos:
            self._count = 0
            windows_elapsed = elapsed // self._window_nanos
            self._window_start = self._window_start + windows_elapsed * self._window_nanos

    def check(self, amount: int, now: Timestamp) -> bool:
        with self._lock:
            self._maybe_reset(now)
            return self._count + amount <= self._capacity

    def record(self, amount: int, now: Timestamp) -> None:
        with self._lock:
            self._maybe_reset(now)
            self._count += amount

    def remaining(self, now: Timestamp) -> int:
        with self._lock:
            self._maybe_reset(now)
            return max(0, self._capacity - self._count)

    def capacity(self) -> int:
        return self._capacity

    def usage_ratio(self, now: Timestamp) -> float:
        cap = self._capacity
        if cap == 0:
            return 1.0
        rem = self.remaining(now)
        return 1.0 - (rem / cap)

    def burn_rate(self, now: Timestamp) -> float:
        with self._lock:
            self._maybe_reset(now)
            elapsed_secs = (now.as_nanos - self._window_start) / 1_000_000_000.0
            if elapsed_secs < 0.001:
                return 0.0
            return self._count / elapsed_secs

    def predicted_exhaustion_secs(self, now: Timestamp) -> float:
        rate = self.burn_rate(now)
        if rate <= 0.0:
            return math.inf
        rem = self.remaining(now)
        return rem / rate

    def reset(self, now: Timestamp) -> None:
        with self._lock:
            self._count = 0
            self._window_start = now.as_nanos


class ConcurrencyLimiter:
    """Concurrency limiter — tracks in-flight requests rather than rate."""

    def __init__(self, cap: int) -> None:
        self._capacity = cap
        self._active = 0
        self._lock = threading.Lock()

    def release(self, amount: int) -> None:
        with self._lock:
            self._active = max(0, self._active - amount)

    def check(self, amount: int, now: Timestamp) -> bool:
        with self._lock:
            return self._active + amount <= self._capacity

    def record(self, amount: int, now: Timestamp) -> None:
        with self._lock:
            self._active += amount

    def remaining(self, now: Timestamp) -> int:
        with self._lock:
            return max(0, self._capacity - self._active)

    def capacity(self) -> int:
        return self._capacity

    def usage_ratio(self, now: Timestamp) -> float:
        cap = self._capacity
        if cap == 0:
            return 1.0
        rem = self.remaining(now)
        return 1.0 - (rem / cap)

    def burn_rate(self, now: Timestamp) -> float:
        return 0.0

    def predicted_exhaustion_secs(self, now: Timestamp) -> float:
        return math.inf

    def reset(self, now: Timestamp) -> None:
        with self._lock:
            self._active = 0


def create_tracker(
    config: "QuotaConfig", now: Timestamp  # noqa: F821
) -> TokenBucket | ConcurrencyLimiter:
    """Create appropriate quota tracker for a given config."""
    from grate_limiter.models import QuotaConfig as QC  # noqa: N811

    assert isinstance(config, QC)
    if config.dimension == Dimension.CONCURRENCY:
        return ConcurrencyLimiter(config.limit)
    window = config.window if config.window is not None else Window.MINUTE
    return TokenBucket(config.limit, window, now)
