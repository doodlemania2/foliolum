# Deploying the editor (app.foliolum.app)

`.github/workflows/deploy.yml` runs after CI passes on `main`, or by hand on `main`
(`gh workflow run deploy.yml`). It never runs for a pull request.

| Step | Where | What |
|---|---|---|
| web | `tdp-linux` | `trunk build` with the size-tuned `web` profile, then `image/prepare.sh dist/web build/image` |
| image | `doodlemanage-foliolum-build` | `docker build` of that context, push to `doodlemanage.tail890bc.ts.net:5000/foliolum-app:sha-<commit>-r<run>-<attempt>`, `smoke-image.sh` on the pushed image |
| deploy | `edge-deploy-foliolum-app` | `edge-ssh sync foliolum-app-stack` with `doodlessd/`, then `edge-ssh run foliolum-app-stack deploy.sh <image>` |

- **image/**: unprivileged nginx serving the web build, with gzip copies beside the large files.
  `prepare.sh` writes the Content-Security-Policy: `'self'` plus the sha256 of the page's one inline
  loader script and one inline style, and `'wasm-unsafe-eval'` so the page can compile its own wasm.
  No other origin.
- **doodlessd/**: the stack in `/home/derek/foliolum-app-stack`, on `127.0.0.1:18117`, reached only
  through the public-fleet Cloudflare tunnel. `deploy.sh` pins the image in `.env`
  (`FOLIOLUM_APP_IMAGE=`), waits for the health check, runs `smoke.sh`, and goes back to the previous
  image if either fails. fleet-mirror copies the compose file and that line to doodlessd2.
- **smoke.sh** checks a running container: health, headers, the wasm's type, caching and gzip, and that
  the CSP holds the hash of the loader the page actually carries. It needs only docker on the host.

Try an image locally: `deploy/image/prepare.sh dist/web /tmp/ctx && docker build -t foliolum-app /tmp/ctx && deploy/smoke-image.sh foliolum-app`.

Not checked automatically yet: the editor starting in a real browser under this CSP, and opening and
exporting a .pub there (checked by hand in Chrome before the first deploy).
