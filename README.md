# Universal Document Render Bot

## Deploy & run

TODO: Update to use ghcr and CI/CD deployment.

On remote host:

```
$ git clone git@github.com:lacop/udrb.git
$ cd udrb
$ sudo docker-compose build
```

Copy over config:

```
$ scp config/config.toml lacop@lacop.dev:udrb/config/config.toml
```

Update & rebuild

```
$ git pull origin master
$ sudo docker-compose build
```

Launch on remote:

```
# Stop if running already
$ sudo docker-compose down -v
$ sudo docker-compose up -d
```

## Local development

### Full Docker

```shell
# Set up reverse SSH tunel to the host configured for slash command (remote 2102 -> local 2101)
$ ssh -N -T -R2102:localhost:2101 lacop.dev

# Run local version.
# TODO change to env file/args to allow overriding stuff here
$ docker compose build && docker compose up
```

### Faster Rust iteration

```shell
# Start chrome container
$ cd browser/
$ docker run -d -p 9222:9222 $(docker build -q .)

# Run the server
$ cd app/
$ ROCKET_PORT=2101 UDRB_OUTPUT_DIR=$PWD/../output UDRB_HOSTNAME=http://udrb-dev.lacop.dev UDRB_CHROME_ADDRESS=127.0.0.1:9222 UDRB_DOMAIN_CONFIG=../config/domains.yaml UDRB_SLACK_MAX_AGE_SECONDS=300 UDRB_SLACK_SECRET=... cargo run
```
