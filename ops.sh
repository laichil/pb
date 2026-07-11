#!/bin/bash
if [[ $1 == "log" ]]; then
    build_log
else
    cargo build --release
fi
cp ./target/release/pb .
echo "🔀 copy..."
cp pb ~/bin
