#!/bin/sh

set -eu

rm -rf deploy/*
npm --prefix web run build -- --outDir ../deploy
cargo run -- --project sbtalker web --export deploy
