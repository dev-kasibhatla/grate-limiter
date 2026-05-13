"""Engine-level metrics counters."""

from __future__ import annotations

import threading


class Metrics:
    """Observable engine metrics. All counters are monotonically increasing."""

    def __init__(self) -> None:
        self._selects = 0
        self._observations = 0
        self._cooldowns_triggered = 0
        self._no_provider_available = 0
        self._lock = threading.Lock()

    @property
    def selects(self) -> int:
        return self._selects

    @property
    def observations(self) -> int:
        return self._observations

    @property
    def cooldowns_triggered(self) -> int:
        return self._cooldowns_triggered

    @property
    def no_provider_available(self) -> int:
        return self._no_provider_available

    def _inc_selects(self) -> None:
        with self._lock:
            self._selects += 1

    def _inc_observations(self) -> None:
        with self._lock:
            self._observations += 1

    def _inc_cooldowns(self) -> None:
        with self._lock:
            self._cooldowns_triggered += 1

    def _inc_no_provider(self) -> None:
        with self._lock:
            self._no_provider_available += 1
