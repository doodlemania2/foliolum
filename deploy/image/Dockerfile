# The editor (app.foliolum.app) as one small image: the web build behind an unprivileged nginx.
# Build from a context that deploy/image/prepare.sh made (it holds the files, gzip copies and the
# CSP with the page's inline hashes):
#
#   deploy/image/prepare.sh dist/web build/image && docker build -t foliolum-app build/image
# nginx's own unprivileged image (github.com/nginx/docker-nginx-unprivileged), from GHCR: Docker Hub's
# nginxinc/ namespace no longer gets releases. 1.31.9 carries the 2026-11 remote-code-execution fixes (#33).
FROM ghcr.io/nginx/nginx-unprivileged:1.31.9-alpine@sha256:60214a103e6f38e953ff957f34b1f2e97a9db801ab2df3821da955d0fe6cadd6
COPY site.conf /etc/nginx/conf.d/default.conf
# Not *.conf: nginx loads every conf.d/*.conf at http level, and these headers belong in the server.
COPY headers.inc /etc/nginx/conf.d/headers.inc
COPY html/ /usr/share/nginx/html/
# The base image already runs as user 101.
