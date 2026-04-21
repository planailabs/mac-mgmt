---
name: Resource Triage
desc: Diagnose and address thermal, memory, disk, and GPU resource issues
---

Diagnose and address system resource issues (thermal, memory, disk, GPU).

## Procedure

1. **Gather resource data**
   - `get_system_sample` — current CPU, memory, disk, GPU, thermal state.
   - `get_inventory` — hardware specs (CPU model, cores, total RAM, GPU model/VRAM).
   - `get_metrics(metric: "cpu_load_1m", hours: 4)` — recent CPU trends.
   - `get_metrics(metric: "memory_used_mb", hours: 4)` — memory trend.

2. **Thermal issues**
   - Sample shows `thermal_state: "serious"` or `"critical"`.
   - This causes CPU throttling, which makes services slow and probes timeout.
   - You cannot fix thermal issues directly — `staff_ping(category: "hardware")` with
     details about the thermal state.
   - Consider reducing load: swap to a smaller model, reduce `OLLAMA_NUM_PARALLEL`.

3. **Memory pressure**
   - High swap usage relative to total RAM indicates memory pressure.
   - **Swap > 10 GB is critical** — functional probes will timeout at 60s because
     model inference becomes extremely slow when paging to/from swap.
   - Check if the loaded model fits in available RAM/VRAM.
   - `run_command("ollama-ps")` to see loaded models and their memory usage.
   - If a model is too large, swap to a smaller one (use `ollama_model_swap` skill).
   - If multiple models are loaded, consider setting `OLLAMA_NUM_PARALLEL=1`.
   - Consider increasing openclaw's `agents.defaults.timeoutSeconds` as a short-term
     mitigation while addressing the root cause.
   - Process count > 5000 combined with high swap indicates the system may be thrashing.
   - `staff_ping(category: "hardware")` with swap usage, thermal state, and process
     count — this typically requires human intervention to free resources.

4. **Disk space**
   - `disk_free` below 5 GB is critical — model pulls will fail, logs may fill up.
   - Check for large unused models: `run_command("ollama-list")` and remove unused ones.
   - `staff_ping(category: "disk_space")` if you can't free enough space.

5. **GPU issues**
   - High VRAM usage with low GPU utilization may indicate a stalled process.
   - GPU temperature above 90°C is concerning — note it in your report.
   - If no GPU is present but the service expects one, check config for CPU-only settings.

## Escalation

Resource issues that you can't resolve by config/model changes require admin intervention.
Always `staff_ping` with the specific resource problem and measurements.
