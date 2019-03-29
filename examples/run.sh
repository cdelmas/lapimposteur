#!/bin/sh

docker run -it --network=host --name lapimposteur -e RUST_LOG=lapimposteur_lib=trace -v $(pwd)/examples:/cnf lapimposteur:0 -c /cnf/config.json