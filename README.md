# Cortex Router

## Build for ARM

1. Build binary: `docker-compose run build-arm7`
2. Copy `target/armv7-unknown-linux-gnueabihf/release/router` to the armv7 based microprocessor.
3. create `init.json` and `config.json` configuration files on the armv7 based microprocessor.
