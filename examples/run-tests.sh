#!/bin/sh
runner --add filetime
runner filetime.rs
runner --add json
runner -C json # from crate in static cache
# dynamic link to json crate
runner json.rs
runner read.rs
runner --add regex
# static build
runner regex.rs
runner --add serde_json
runner serde_json.rs
# shared lib from rust file
runner -C universe.rs
runner -Xuniverse -e 'answer()'
runner --add tokio=1.0/full
runner async/async.rs
