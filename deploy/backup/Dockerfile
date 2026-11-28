# The database's daily backup: pg_dump 17 (matches foliolum-db) plus restic, in one small image.
# From doodlemania2/chores deploy/doodlessd/backup, built by CI on doodlemanage (deploy.yml) rather
# than on the edge host, and pinned like every other image.
FROM postgres:17-alpine@sha256:18cfe3ef5e6815560c98237d6216d1e5119702fb0f3894c8785dd58b8bbe5d73
RUN apk add --no-cache restic curl tzdata
COPY backup.sh /usr/local/bin/backup.sh
COPY loop.sh /usr/local/bin/loop.sh
RUN chmod +x /usr/local/bin/*.sh
USER postgres
ENTRYPOINT ["/usr/local/bin/loop.sh"]
