#!/bin/bash

rdx() {
    echo "\$" "$@"
    "$@"
}

rdx cargo build --verbose || exit 1
echo
rdx cd event || exit 1
rdx cargo test --tests --verbose --features "crossbeam-channel"
