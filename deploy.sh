#!/bin/sh

set -eu

rm -rf deploy/*
npm --prefix web run build -- --outDir ../deploy
cargo run -- --project project/blaster web --export deploy
