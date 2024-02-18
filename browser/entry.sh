#!/bin/bash

# Forward port because --remote-debugging-address=0.0.0.0 doesn't work
# without --headless apparently.
socat tcp-l:${DEBUG_PORT},fork,reuseaddr tcp:localhost:1234 &

xvfb-run \
    chromium \
    --no-sandbox \
    --disable-gpu \
    --disable-setuid-sandbox \
    --disable-dev-shm-usage \
    --user-data-dir=${HOME} \
    --remote-debugging-port=1234
