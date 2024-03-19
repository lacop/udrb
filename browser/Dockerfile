FROM debian:stable
RUN apt-get update && apt-get install -y chromium dumb-init netcat-openbsd socat xvfb

COPY entry.sh /usr/bin/

RUN mkdir /data
VOLUME /data

EXPOSE 9222
EXPOSE 9999

ENV HOME=/data DEBUG_PORT=9222 KILL_PORT=9999

ENTRYPOINT ["/usr/bin/dumb-init", "--"]
CMD [ "/usr/bin/entry.sh" ]
