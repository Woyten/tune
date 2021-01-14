#! /usr/bin/env bash

set -eux

RUSTFLAGS=--cfg=web_sys_unstable_apis trunk serve