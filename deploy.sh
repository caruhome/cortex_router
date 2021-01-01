#!/bin/bash

rsync --progress target/armv7-unknown-linux-gnueabihf/release/router $1:/tmp
rsync --progress deployment/init.json $1:/tmp
rsync --progress deployment/config.json $1:/tmp
