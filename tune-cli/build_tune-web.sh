#!/usr/bin/env bash

set -eux

rm -r dist
trunk build --release
