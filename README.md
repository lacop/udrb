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

Launch on remote:
```
$ sudo docker-compose up -d
```