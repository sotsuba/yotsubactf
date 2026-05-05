FROM prom/alertmanager:v0.27.0 AS alertmanager_bin

FROM alpine:3.19

RUN apk add --no-cache gettext libc6-compat

COPY --from=alertmanager_bin /bin/alertmanager /bin/alertmanager
COPY --from=alertmanager_bin /bin/amtool /bin/amtool

# Copy all config variants
COPY . /etc/alertmanager/


RUN echo '#!/bin/sh' > /entrypoint.sh && \
    echo 'if [ -f "/etc/alertmanager/alertmanager.${ENV:-dev}.yml.tmpl" ]; then' >> /entrypoint.sh && \
    echo '  envsubst < "/etc/alertmanager/alertmanager.${ENV:-dev}.yml.tmpl" > /etc/alertmanager/alertmanager.yml' >> /entrypoint.sh && \
    echo 'else' >> /entrypoint.sh && \
    echo '  cp "/etc/alertmanager/alertmanager.${ENV:-dev}.yml" /etc/alertmanager/alertmanager.yml' >> /entrypoint.sh && \
    echo 'fi' >> /entrypoint.sh && \
    echo 'exec /bin/alertmanager "$@"' >> /entrypoint.sh && \
    chmod +x /entrypoint.sh

EXPOSE 9093
VOLUME [ "/alertmanager" ]

ENTRYPOINT ["/entrypoint.sh"]
CMD ["--config.file=/etc/alertmanager/alertmanager.yml", "--storage.path=/alertmanager"]