"""Tests for clock module."""

from grate_limiter.clock import MockClock, RealClock, Timestamp


class TestTimestamp:
    def test_zero(self) -> None:
        assert Timestamp.ZERO.as_nanos == 0
        assert Timestamp.ZERO.as_millis == 0

    def test_conversions(self) -> None:
        ts = Timestamp(5_000_000_000)
        assert ts.as_millis == 5000
        assert abs(ts.as_secs_f64 - 5.0) < 0.001

    def test_duration_since(self) -> None:
        a = Timestamp(10_000_000)
        b = Timestamp(3_000_000)
        assert a.duration_since(b) == 7_000_000
        assert b.duration_since(a) == 0  # clamped to 0

    def test_add_methods(self) -> None:
        ts = Timestamp(0)
        assert ts.add_millis(100).as_millis == 100
        assert ts.add_secs(2).as_millis == 2000
        assert ts.add_nanos(1_000_000).as_millis == 1

    def test_comparison(self) -> None:
        a = Timestamp(100)
        b = Timestamp(200)
        assert a < b
        assert b > a
        assert a <= a
        assert a == Timestamp(100)


class TestRealClock:
    def test_monotonic(self) -> None:
        clock = RealClock()
        t1 = clock.now()
        t2 = clock.now()
        assert t2 >= t1


class TestMockClock:
    def test_starts_at_zero(self) -> None:
        clock = MockClock()
        assert clock.now().as_millis == 0

    def test_advance_ms(self) -> None:
        clock = MockClock()
        clock.advance_ms(5000)
        assert clock.now().as_millis == 5000

    def test_advance_secs(self) -> None:
        clock = MockClock()
        clock.advance_secs(3)
        assert clock.now().as_millis == 3000

    def test_at(self) -> None:
        clock = MockClock.at(Timestamp(10_000_000_000))
        assert clock.now().as_millis == 10000

    def test_set(self) -> None:
        clock = MockClock()
        clock.set(Timestamp(42_000_000))
        assert clock.now().as_millis == 42
