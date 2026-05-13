"""Enums for quota dimensions, time windows, and status classes."""

from __future__ import annotations

from enum import Enum


class Dimension(Enum):
    """Quota dimension — what resource is being tracked."""

    REQUESTS = "requests"
    TOKENS = "tokens"
    CONCURRENCY = "concurrency"
    COST_USD = "cost_usd"
    BYTES = "bytes"


class Window(Enum):
    """Time window for quota reset."""

    SECOND = "second"
    MINUTE = "minute"
    HOUR = "hour"
    DAY = "day"

    def as_nanos(self) -> int:
        return _WINDOW_NANOS[self]

    def as_secs(self) -> int:
        return _WINDOW_SECS[self]


_WINDOW_NANOS = {
    Window.SECOND: 1_000_000_000,
    Window.MINUTE: 60_000_000_000,
    Window.HOUR: 3_600_000_000_000,
    Window.DAY: 86_400_000_000_000,
}

_WINDOW_SECS = {
    Window.SECOND: 1,
    Window.MINUTE: 60,
    Window.HOUR: 3_600,
    Window.DAY: 86_400,
}


class StatusClass(Enum):
    """Classified response status for health tracking."""

    SUCCESS = "success"
    RATE_LIMITED = "rate_limited"
    FORBIDDEN = "forbidden"
    SERVER_ERROR = "server_error"
    TIMEOUT = "timeout"
    CLIENT_ERROR = "client_error"
