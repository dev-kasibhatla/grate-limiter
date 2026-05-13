"""Exception types for grate-limiter."""

from __future__ import annotations


class GrateLimiterError(Exception):
    """Base exception for all grate-limiter errors."""


class UnknownCapability(GrateLimiterError):
    """The requested capability does not exist."""

    def __init__(self, name: str) -> None:
        self.name = name
        super().__init__(f"unknown capability: {name}")


class UnknownProvider(GrateLimiterError):
    """The referenced provider does not exist."""

    def __init__(self, name: str) -> None:
        self.name = name
        super().__init__(f"unknown provider: {name}")


class NoAvailableProviders(GrateLimiterError):
    """No providers are available for the requested capability."""

    def __init__(self, capability: str) -> None:
        self.capability = capability
        super().__init__(f"no available providers for capability: {capability}")
