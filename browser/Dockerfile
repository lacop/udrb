FROM debian:stable
RUN apt-get update && apt-get install -y dumb-init chromium socat xvfb

COPY entry.sh /usr/bin/

RUN mkdir /data
VOLUME /data

EXPOSE 9222

ENV HOME=/data DEBUG_PORT=9222

ENTRYPOINT ["/usr/bin/dumb-init", "--"]
CMD [ "/usr/bin/entry.sh" ]
