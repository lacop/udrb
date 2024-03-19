#!/bin/bash

# Make sure restarts are clean.
killall socat || true
killall chromium || true
killall xvfb-run || true
killall Xvfb || true
rm -rf /tmp/.X99-lock || true
rm -rf /tmp/.org.chromium.Chromium.* || true

# Forward port because --remote-debugging-address=0.0.0.0 doesn't work
# without --headless apparently.
socat tcp-l:${DEBUG_PORT},fork,reuseaddr tcp:localhost:1234 &

xvfb-run -e /dev/stdout \
    chromium \
    --no-sandbox \
    --disable-gpu \
    --disable-setuid-sandbox \
    --disable-dev-shm-usage \
    --user-data-dir=${HOME} \
    --remote-debugging-port=1234

# TODO: Auto-restart the process (or whole container) when rendering fails.
#       Eg. we could have a second socat that listens on "kill port" and 
#       kills the container whenever it gets a request. Docker will then
#       restart it, and we just need the Rust app to send a kill request as needed.