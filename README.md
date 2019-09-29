# Universal Document Render Bot

## Deploy & run

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

```
# Set up reverse SSH tunel to the host configured for slash command (remote 2102 -> local 2101)
$ ssh -f -N -T -R2102:localhost:2101 lacop.dev
# Run local version.
# TODO change to env file/args to allow overriding stuff here
$ docker-compose build && docker-compose up
```