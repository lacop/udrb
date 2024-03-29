# Universal Document Render Bot

Slack slash command that allows users to capture a URL and post a screenshot, PDF and MHTML archive of the page.

## Slack configuration

* Create new app
* Get the "Signing secret" and set it in the `.env` file
* Add a "Slash command" called `/udrb` with request url `https://hostname/slack/slash`, and some description and usage hint.
* In "Interactivity & Shortcuts" enable "Interactivity and add a new "Request URL" `https://hostname/slack/interactive`.

## Deploy & run

Optionally build the image locally, or wait for github actions to rebuild. Building on the VPS is painfully slow.

```shell
$ (cd app/ && docker build -t ghcr.io/lacop/udrb-app . && docker push ghcr.io/lacop/udrb-app)
$ (cd browser/ && docker build -t ghcr.io/lacop/udrb-chrome . && docker push ghcr.io/lacop/udrb-chrome)
```

On remote host, update the repo and images:

```shell
# First time: git clone git@github.com:lacop/udrb.git
$ cd udrb
$ git pull origin master

# Fast
$ sudo docker compose pull

# Slow
$ sudo docker compose build 
```

Copy over config, if it has changed:

```shell
$ scp config/.env lacop@lacop.dev:udrb/config/
$ scp config/domains.yaml lacop@lacop.dev:udrb/config/
```

Launch on remote:

```shell
# Stop if running already
$ sudo docker compose down -v
$ sudo docker compose up -d
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

## TODO

* More robust handling of chromium hanging - timeouts etc.
* Add `Referrer-Policy: no-referrer` to the `/static` handler.
