#!/bin/bash

rm -rf db/database.test.db
cargo test
rm -rf db/database.test.db