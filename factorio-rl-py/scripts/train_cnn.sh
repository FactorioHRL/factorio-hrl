#!/usr/bin/env bash
# The CNN phases of one task, one after the other (each continues the best model of the previous one).
#   scripts/train_cnn.sh <automation_science|production_science> [extra arguments, e.g. --live]
#   PHASES="2 3" scripts/train_cnn.sh production_science      # only some phases (default: 1 2 3)
# Run it with the virtualenv active (or set PYTHON). Extra arguments go to every phase. A phase that
# continues an earlier one stops with a clear message when that one has not produced a model yet.
set -euo pipefail
cd "$(dirname "$0")/.."

KEY="${1:?usage: scripts/train_cnn.sh <automation_science|production_science> [extra arguments]}"
shift
DUMP="${FSIM_DUMP:-../data/data-raw-dump.json}"
PYTHON="${PYTHON:-python}"
PHASES="${PHASES:-1 2 3}"

# phases 1 and 2 train on the plain task, phase 3 where machines nothing can reach make nothing
task_of() {
  case "$1" in
    1 | 2) echo "${KEY}_target_output" ;;
    3) echo "${KEY}_target_output_access" ;;
    *) echo "unknown phase '$1' (use 1, 2 or 3)" >&2; return 1 ;;
  esac
}

for phase in $PHASES; do task_of "$phase" > /dev/null; done  # check them all before training anything

for phase in $PHASES; do
  "$PYTHON" -m factorio_rl.train \
    --task "configs/tasks/$(task_of "$phase").toml" \
    --training "configs/training/cnn_${KEY}_p${phase}.toml" \
    --dump "$DUMP" "$@"
done
