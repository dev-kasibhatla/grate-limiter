"""Tests for quota strategies."""

from grate_limiter.clock import Timestamp
from grate_limiter.quota import ConcurrencyLimiter, FixedWindow, SlidingWindowCounter, TokenBucket
from grate_limiter.types import Window


def ts(ms: int) -> Timestamp:
    return Timestamp(ms * 1_000_000)


class TestTokenBucket:
    def test_new_bucket_is_full(self) -> None:
        bucket = TokenBucket(100, Window.MINUTE, ts(0))
        assert bucket.remaining(ts(0)) == 100
        assert bucket.check(100, ts(0))
        assert not bucket.check(101, ts(0))

    def test_consume_reduces_remaining(self) -> None:
        bucket = TokenBucket(100, Window.MINUTE, ts(0))
        bucket.record(30, ts(0))
        assert bucket.remaining(ts(0)) == 70

    def test_tokens_refill_over_time(self) -> None:
        bucket = TokenBucket(60, Window.MINUTE, ts(0))
        bucket.record(60, ts(0))
        assert bucket.remaining(ts(0)) == 0
        assert bucket.remaining(ts(30_000)) == 30
        assert bucket.remaining(ts(60_000)) == 60

    def test_never_exceeds_capacity(self) -> None:
        bucket = TokenBucket(100, Window.MINUTE, ts(0))
        assert bucket.remaining(ts(120_000)) == 100

    def test_burn_rate_tracks_consumption(self) -> None:
        bucket = TokenBucket(100, Window.MINUTE, ts(0))
        bucket.record(10, ts(1_000))
        bucket.record(10, ts(2_000))
        rate = bucket.burn_rate(ts(5_000))
        assert abs(rate - 4.0) < 0.5

    def test_usage_ratio(self) -> None:
        bucket = TokenBucket(100, Window.MINUTE, ts(0))
        assert abs(bucket.usage_ratio(ts(0))) < 0.01
        bucket.record(80, ts(0))
        assert abs(bucket.usage_ratio(ts(0)) - 0.8) < 0.01

    def test_predicted_exhaustion(self) -> None:
        bucket = TokenBucket(100, Window.MINUTE, ts(0))
        bucket.record(50, ts(5_000))
        secs = bucket.predicted_exhaustion_secs(ts(5_000))
        assert abs(secs - 5.0) < 1.0

    def test_reset_restores_full_capacity(self) -> None:
        bucket = TokenBucket(100, Window.MINUTE, ts(0))
        bucket.record(100, ts(0))
        assert bucket.remaining(ts(0)) == 0
        bucket.reset(ts(1_000))
        assert bucket.remaining(ts(1_000)) == 100


class TestSlidingWindowCounter:
    def test_new_window_has_full_capacity(self) -> None:
        sw = SlidingWindowCounter(100, Window.MINUTE, ts(0))
        assert sw.remaining(ts(0)) == 100

    def test_record_reduces_remaining(self) -> None:
        sw = SlidingWindowCounter(100, Window.MINUTE, ts(0))
        sw.record(40, ts(0))
        assert sw.remaining(ts(0)) == 60

    def test_window_rotation_interpolates(self) -> None:
        sw = SlidingWindowCounter(100, Window.MINUTE, ts(0))
        sw.record(80, ts(0))
        remaining = sw.remaining(ts(90_000))
        assert 55 <= remaining <= 65, f"remaining={remaining}"

    def test_full_window_resets(self) -> None:
        sw = SlidingWindowCounter(100, Window.MINUTE, ts(0))
        sw.record(100, ts(0))
        assert sw.remaining(ts(0)) == 0
        assert sw.remaining(ts(120_001)) == 100


class TestFixedWindow:
    def test_new_window_full_capacity(self) -> None:
        fw = FixedWindow(100, Window.MINUTE, ts(0))
        assert fw.remaining(ts(0)) == 100

    def test_record_reduces_remaining(self) -> None:
        fw = FixedWindow(100, Window.MINUTE, ts(0))
        fw.record(60, ts(0))
        assert fw.remaining(ts(0)) == 40

    def test_window_resets_after_expiry(self) -> None:
        fw = FixedWindow(100, Window.MINUTE, ts(0))
        fw.record(100, ts(0))
        assert fw.remaining(ts(0)) == 0
        assert fw.remaining(ts(60_000)) == 100

    def test_check_respects_capacity(self) -> None:
        fw = FixedWindow(100, Window.MINUTE, ts(0))
        assert fw.check(100, ts(0))
        assert not fw.check(101, ts(0))
        fw.record(90, ts(0))
        assert fw.check(10, ts(0))
        assert not fw.check(11, ts(0))


class TestConcurrencyLimiter:
    def test_new_has_full_capacity(self) -> None:
        cl = ConcurrencyLimiter(10)
        assert cl.remaining(ts(0)) == 10

    def test_record_occupies_slots(self) -> None:
        cl = ConcurrencyLimiter(10)
        cl.record(3, ts(0))
        assert cl.remaining(ts(0)) == 7

    def test_release_frees_slots(self) -> None:
        cl = ConcurrencyLimiter(10)
        cl.record(5, ts(0))
        assert cl.remaining(ts(0)) == 5
        cl.release(3)
        assert cl.remaining(ts(0)) == 8

    def test_check_respects_capacity(self) -> None:
        cl = ConcurrencyLimiter(5)
        assert cl.check(5, ts(0))
        assert not cl.check(6, ts(0))
        cl.record(3, ts(0))
        assert cl.check(2, ts(0))
        assert not cl.check(3, ts(0))

    def test_time_does_not_affect_concurrency(self) -> None:
        cl = ConcurrencyLimiter(10)
        cl.record(10, ts(0))
        assert cl.remaining(ts(0)) == 0
        assert cl.remaining(ts(60_000)) == 0
