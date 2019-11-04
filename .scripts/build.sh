#!/bin/bash

cargo build --verbose --all --examples
cargo test --verbose --all-features
