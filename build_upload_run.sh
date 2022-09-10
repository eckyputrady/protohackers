#!/bin/bash

set -e

echo "Building the project"
cargo build --release
echo "-----"

echo "SCP-ing the release to" $1
scp -i ~/.ssh/id_rsa target/release/protohackers $1:protohackers_new
echo "-----"

echo "Run the new release on" $1
ssh -i ~/.ssh/id_rsa $1 "
    pkill protohackers
    rm protohackers
    mv protohackers_new protohackers
    nohup ./protohackers &
    sleep 1
    ss -lnt
    exit
"
echo "-----"