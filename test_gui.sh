#!/bin/bash
env DISPLAY=:0 WAYLAND_DISPLAY=wayland-0 cargo run -- test_file.bin </dev/null &
PID=$!
sleep 2
kill $PID || true
