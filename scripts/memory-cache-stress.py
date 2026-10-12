import ctypes
import gc
import json
from ctypes import wintypes

import visual_novel_engine as native
from vnengine.types import SCRIPT_SCHEMA_VERSION


class MemoryCounters(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in [
            "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage",
            "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage",
            "PagefileUsage", "PeakPagefileUsage", "PrivateUsage",
        ]
    ]


get_process = ctypes.windll.kernel32.GetCurrentProcess
get_process.restype = wintypes.HANDLE
get_memory = ctypes.windll.psapi.GetProcessMemoryInfo
get_memory.argtypes = [wintypes.HANDLE, ctypes.POINTER(MemoryCounters), wintypes.DWORD]
get_memory.restype = wintypes.BOOL


def memory():
    counters = MemoryCounters()
    counters.cb = ctypes.sizeof(counters)
    if not get_memory(get_process(), ctypes.byref(counters), counters.cb):
        raise ctypes.WinError()
    return {"working_set_bytes": counters.WorkingSetSize, "private_bytes": counters.PrivateUsage}


engine = native.Engine(json.dumps({
    "script_schema_version": SCRIPT_SCHEMA_VERSION,
    "labels": {"start": 0},
    "events": [{"type": "dialogue", "speaker": "Narrator", "text": "Memory smoke"}],
}))
budget = 1024 * 1024
engine.set_resources(native.ResourceConfig(max_texture_memory=budget))
payload = bytes(8192)
samples = []
for batch in range(5):
    for index in range(10_000):
        engine.cache_asset(f"asset/{batch}-{index}.bin", payload)
    gc.collect()
    stats = engine.asset_cache_stats()
    assert stats["allocated_bytes"] <= budget
    assert stats["total_cached_assets"] <= 4096
    samples.append({"operations": (batch + 1) * 10_000, "cache": stats, **memory()})
engine.clear_asset_cache()
assert engine.asset_cache_stats()["total_cached_assets"] == 0
assert engine.asset_cache_stats()["allocated_bytes"] == 0
print(json.dumps({"native_module": native.__file__, "budget_bytes": budget,
                  "samples": samples, "after_clear": engine.asset_cache_stats()}, indent=2))
