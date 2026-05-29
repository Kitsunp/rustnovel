import pytest

from native_healthcheck import python_native_module_origin_healthcheck


def _ensure_local_native_module(config):
    healthcheck = getattr(config, "_vnengine_native_healthcheck", None)
    if healthcheck is None:
        healthcheck = python_native_module_origin_healthcheck()
        config._vnengine_native_healthcheck = healthcheck
    if not healthcheck["ok"]:
        raise pytest.UsageError(healthcheck["message"])


def pytest_sessionstart(session):
    _ensure_local_native_module(session.config)


def pytest_configure(config):
    config._vnengine_pytest_conftest_loaded = True


@pytest.fixture
def vnengine_pytest_conftest_loaded(request):
    return bool(getattr(request.config, "_vnengine_pytest_conftest_loaded", False))


def pytest_collection_modifyitems(config, items):
    _ensure_local_native_module(config)
    skipped = [
        item.nodeid
        for item in items
        if any(mark.name in {"skip", "skipif"} for mark in item.iter_markers())
    ]
    if skipped:
        joined = "\n".join(f"- {nodeid}" for nodeid in skipped)
        raise pytest.UsageError(f"Skipped Python tests are not allowed:\n{joined}")


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    report = outcome.get_result()
    if report.skipped:
        report.outcome = "failed"
        report.longrepr = f"Skipped Python tests are not allowed: {item.nodeid}"
