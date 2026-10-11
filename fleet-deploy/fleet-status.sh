#!/bin/sh
# One-shot view of the whole fleet: the services, the runner's cells and the
# Incus instances behind them.
set -u
echo "== services =="
/var/lib/reagent/fleet/check.sh
echo
echo "== runner cells =="
if curl -fsS -m 30 http://127.0.0.1:9400/status -o /tmp/fleet-status.json 2>/dev/null; then
  python3 - <<'PY'
import json
d = json.load(open('/tmp/fleet-status.json'))
print(f"running {d['running']}/{d['total_cells']}, paused={d['paused']}")
for c in d['cells']:
    print(f"  {c['key']:<22} {c['stage']:<14} nodes={len(c.get('instances') or [])} err={c.get('last_error')}")
PY
else
  echo "  runner did not answer"
fi
echo
echo "== incus instances (reagent-fleet) =="
/var/lib/reagent/fleet/reagent-fleet-ls.sh
