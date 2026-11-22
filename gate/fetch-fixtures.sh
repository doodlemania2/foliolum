#!/usr/bin/env bash
# Fetches the gate's public development set into fixtures/ (gitignored, never committed):
#
#   fixtures/pubsmith/   FuncularLabs/pubsmith's tests/fixtures/pub at the pin below: 142 .pub files made
#                        with Publisher by that project, each with Publisher's layout export in truth/.
#   fixtures/dev22/      the spike's 22 public test files: 21 from Apache POI's test-data/publisher
#                        (Apache-2.0) and one from LibreOffice's libmspub tests (MPL-2.0), at the pins below.
#                        No layout exports, so the gate checks them against Pubwork's reader.
#   fixtures/pubwork/    Pubwork's patched libmspub WebAssembly reader (Daffyllas/Pubwork, MPL-2.0): the
#                        gate's second reference (gate/pubwork/oracle.mjs runs it under Node).
#
# pubsmith is MIT, but its THIRD-PARTY-NOTICES.md says these files carry Microsoft material outside that
# licence, so we fetch them for local testing and never vendor them. Their truth JSON scores imports
# only; it is never used to work out the file format.
#
# Then: cargo xtask gate --set pubsmith --names fixtures/pubsmith
#       cargo xtask gate --set dev22 --names fixtures/dev22
#
# Every file fetched by URL is checked against its SHA-256 below.
set -euo pipefail

PUBSMITH_REPO=https://github.com/FuncularLabs/pubsmith.git
PUBSMITH_COMMIT=0ccdf27fc3f381f0897515e226ea485165c699b2

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLONE="$ROOT/fixtures/.pubsmith-src"
DEST="$ROOT/fixtures/pubsmith"

if [[ ! -d "$CLONE/.git" ]]; then
  echo "==> cloning pubsmith (fixtures only)"
  mkdir -p "$ROOT/fixtures"
  git clone --quiet --filter=blob:none --no-checkout --sparse "$PUBSMITH_REPO" "$CLONE"
  git -C "$CLONE" sparse-checkout set tests/fixtures/pub
fi
git -C "$CLONE" cat-file -e "$PUBSMITH_COMMIT^{commit}" 2>/dev/null || git -C "$CLONE" fetch --quiet origin "$PUBSMITH_COMMIT"
git -C "$CLONE" -c advice.detachedHead=false checkout --quiet --force "$PUBSMITH_COMMIT"
actual="$(git -C "$CLONE" rev-parse HEAD)"
[[ "$actual" == "$PUBSMITH_COMMIT" ]] || { echo "!! pubsmith is at $actual, expected $PUBSMITH_COMMIT" >&2; exit 1; }

# Relative, so the link survives the folder being moved (another worktree, another machine).
ln -sfn ".pubsmith-src/tests/fixtures/pub" "$DEST"
echo "==> $(find -L "$DEST" -maxdepth 1 -name '*.pub' | wc -l | tr -d ' ') .pub files, $(find -L "$DEST/truth" -name '*.json' | wc -l | tr -d ' ') truth exports in $DEST"

POI_COMMIT=c94ee8ad9096bdb8cf8e5e59d8d12432e950dbf3
LO_COMMIT=e2dc267756d88e979036b1a5715d8c585a9e85e6
PUBWORK_COMMIT=8dd03e111f8f477ab1c1c005e28c80d971647850

# sha256sum on Linux, shasum on macOS.
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# fetch URL DEST SHA256: download unless DEST already has that hash, then check it.
fetch() {
  local url="$1" dest="$2" want="$3"
  if [[ ! -f "$dest" ]] || [[ "$(sha256 "$dest")" != "$want" ]]; then
    curl -fsSL --retry 3 -o "$dest.part" "$url"
    mv "$dest.part" "$dest"
  fi
  local got
  got="$(sha256 "$dest")"
  [[ "$got" == "$want" ]] || { echo "!! $dest: SHA-256 $got, expected $want" >&2; rm -f "$dest"; exit 1; }
}

echo "==> dev22 (Apache POI at ${POI_COMMIT:0:12}, LibreOffice at ${LO_COMMIT:0:12})"
mkdir -p "$ROOT/fixtures/dev22"
while read -r name sha; do
  fetch "https://raw.githubusercontent.com/apache/poi/$POI_COMMIT/test-data/publisher/$name.pub" "$ROOT/fixtures/dev22/poi-$name.pub" "$sha"
done <<'LIST'
51318 3ab75a6a9196e0a51fc9b0aa759459501c71d313030c06652aabffbae0a2ab09
60685 0d3cecf0ff000a20931d0de3ebc48040bea10e03833ade3f9902f996c20e7184
LinkAt0And10 c59b7160c21a531e5872e76141c4cd43a4d74f30d78401f64175467fa40c44fc
LinkAt10 f1b7f65cb9646fddc2cfc77a795109bb958f629e4fbf7c9fc7a4d1b045074c35
LinkAt10And20And30 202ac6f82e2a33ef0181158728b5c4df6781ccb105e012f520d331acafbcebf1
LinkAt10And20And30And40 b54068d4f82dd0ee533f2a655a7edd2acbdc02619621b32fb23cb9ce842ed669
LinkAt10Longer a917a9e575f5cfc1ba8e78226981b662b06f96da2d7f2eb3e2b00a4b4f3dada5
LinkAt20 cec79a2bd2f6bc71dd276bd6584c34d66fa6f710d9adbed951bd9cb32c5b2fed
Sample 6fefdef46b87c767150878dc384549cb2d2ec2ac54de25f8ddb3a5628301107e
Sample2 c87ea7cc5606023c5fa99376631bbd45a8e9bdaba34c958057995b0e7ac8cc86
Sample2000 40701ca47b26d04771cdd58467764e9ab39d3da69b59fbc529afd16d263d2f86
Sample2_2010 931ff20980b8f72aab227fa3cbe61b017be8d17c337fecb9aca3c97075940777
Sample3 424c69173ff08948c2529c8084b4ac2403f1ff1057146f4edd02fc29b44481fc
Sample3_2010 baa9555a72d9e4264e3fd722896e2e43b960d20b4afdb96f75cb721a6133ff91
Sample4 42195f7ad23d911219fea3ec88e66e867e9b9a6821a16dd1b535e2aa9d57a11b
Sample4_2010 544e2043f91aed664c0a09f9ae45da42291b6730f107f99f3cdf9462ed38fb02
Sample98 8912f295ff0cd1f55b873b3b98a182218f33c420877e025d2f0fd795f12dd0c0
SampleBrochure ffed034ac87e679f0bd08ff9cf74ad11c0e0e510a42b1bc1a7502415f6c29c87
SampleNewsletter 6a825ba26ba35d6e885acdc62e859591ed37cb0ff7480b554b9cb362b644dfcf
Sample_2010 f0a90aa566d72eb822eb48b4f378c33007610e35f3f1b999cdb88f2c9683858b
Simple 2606f530052a818c7949b88c9bf245dbc0a74ddc6b5a60004f37b387fe4b955d
LIST
fetch "https://raw.githubusercontent.com/LibreOffice/core/$LO_COMMIT/writerperfect/qa/unit/data/draw/libmspub/pass/fdo59355-1.pub" \
  "$ROOT/fixtures/dev22/lo-fdo59355-1.pub" 79f9a8d99ed18faa174fa7200dbe91627015e2b77439a20421119abc128d62f7
echo "==> $(find "$ROOT/fixtures/dev22" -name '*.pub' | wc -l | tr -d ' ') .pub files in $ROOT/fixtures/dev22"

echo "==> Pubwork's reader at ${PUBWORK_COMMIT:0:12}"
mkdir -p "$ROOT/fixtures/pubwork"
PW="https://raw.githubusercontent.com/Daffyllas/Pubwork/$PUBWORK_COMMIT"
fetch "$PW/public/pubreader/pubreader.mjs" "$ROOT/fixtures/pubwork/pubreader.mjs" 48830873c9d8141e9da679e1847010111077f01150835acf51e31f8f1f63a0c6
fetch "$PW/public/pubreader/pubreader.wasm" "$ROOT/fixtures/pubwork/pubreader.wasm" 39f541c1cd7a53f669d72f8c6ec113170de2a90a3b75c0ef03d1fdddee0dfc56
fetch "$PW/src/pubmodel.js" "$ROOT/fixtures/pubwork/pubmodel.js" e30e05e3db6d37f5feb1125164e27af8d376c05c158558b384ded5b93edbfbd4
fetch "$PW/LICENSE" "$ROOT/fixtures/pubwork/LICENSE" fab3dd6bdab226f1c08630b1dd917e11fcb4ec5e1e020e2c16f83a0a13863e85
echo "==> Pubwork reader in $ROOT/fixtures/pubwork"
