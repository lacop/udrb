#!/bin/bash

VENDOR_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
echo "Working in ${VENDOR_DIR}"

# chrome.json: seccomp policy for chrome docker image
wget https://raw.githubusercontent.com/jfrazelle/dotfiles/master/etc/docker/seccomp/chrome.json -O "${VENDOR_DIR}/chrome.json"
