# The editor (app.foliolum.app) as one small image: the web build behind an unprivileged nginx.
# Build from a context that deploy/image/prepare.sh made (it holds the files, gzip copies and the
# CSP with the page's inline hashes):
#
#   deploy/image/prepare.sh dist/web build/image && docker build -t foliolum-app build/image
FROM nginxinc/nginx-unprivileged:1.29-alpine@sha256:0c79d56aee561a1d81c63f00eee5fb5fe29279560cdc55e91425133104c7fbe6
COPY site.conf /etc/nginx/conf.d/default.conf
# Not *.conf: nginx loads every conf.d/*.conf at http level, and these headers belong in the server.
COPY headers.inc /etc/nginx/conf.d/headers.inc
COPY html/ /usr/share/nginx/html/
# The base image already runs as user 101.
