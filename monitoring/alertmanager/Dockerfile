FROM prom/alertmanager:v0.27.0 AS alertmanager_bin

FROM alpine:3.19

RUN apk add --no-cache gettext libc6-compat

COPY --from=alertmanager_bin /bin/alertmanager /bin/alertmanager
COPY --from=alertmanager_bin /bin/amtool /bin/amtool

# Configs will be mounted to /config at runtime
RUN mkdir -p /etc/alertmanager /config

RUN echo '#!/bin/sh' > /entrypoint.sh && \
    echo 'if [ -f "/config/alertmanager.${ENV:-dev}.yml.tmpl" ]; then' >> /entrypoint.sh && \
    echo '  envsubst < "/config/alertmanager.${ENV:-dev}.yml.tmpl" > /etc/alertmanager/alertmanager.yml' >> /entrypoint.sh && \
    echo 'elif [ -f "/config/alertmanager.${ENV:-dev}.yml" ]; then' >> /entrypoint.sh && \
    echo '  cp "/config/alertmanager.${ENV:-dev}.yml" /etc/alertmanager/alertmanager.yml' >> /entrypoint.sh && \
    echo 'else' >> /entrypoint.sh && \
    echo '  echo "Error: No config found for ENV=${ENV:-dev} in /config"' >> /entrypoint.sh && \
    echo '  exit 1' >> /entrypoint.sh && \
    echo 'fi' >> /entrypoint.sh && \
    echo 'exec /bin/alertmanager "$@"' >> /entrypoint.sh && \
    chmod +x /entrypoint.sh

EXPOSE 9093
VOLUME [ "/alertmanager" ]

ENTRYPOINT ["/entrypoint.sh"]
CMD ["--config.file=/etc/alertmanager/alertmanager.yml", "--storage.path=/alertmanager"]