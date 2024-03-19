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
    --remote-debugging-port=1234 &
CHROME_PID=$!

# Wait for connection on the kill port. If we get one exit the container,
# so Docker restarts us. Try to kill chromium cleanly before that if we can.
nc -l ${KILL_PORT} -N < /dev/null
kill ${CHROME_PID}
sleep 1
kill -9 ${CHROME_PID} || true

