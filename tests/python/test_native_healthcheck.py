from native_healthcheck import (
    EXPECTED_PUBLIC_NAMES,
    python_native_module_origin_healthcheck,
)


def test_python_native_module_origin_healthcheck():
    healthcheck = python_native_module_origin_healthcheck()

    assert healthcheck["ok"], healthcheck["message"]
    assert healthcheck["sys_prefix"]
    assert healthcheck["sys_base_prefix"]
    assert healthcheck["module_file"]
    assert not healthcheck["missing_public_names"]
    assert EXPECTED_PUBLIC_NAMES.issubset(set(healthcheck["public_names"]))
