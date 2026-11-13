#!/usr/bin/env bash
# The three CNN phases for automation_science; see scripts/train_cnn.sh (PHASES="3" runs only the third).
exec "$(dirname "$0")/train_cnn.sh" automation_science "$@"
