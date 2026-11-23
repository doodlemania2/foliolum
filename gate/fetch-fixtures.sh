#!/usr/bin/env bash
# Fetches the gate's public development set into fixtures/ (gitignored, never committed):
#
#   fixtures/pubsmith/   FuncularLabs/pubsmith's tests/fixtures/pub at the pin below: 142 .pub files made
#                        with Publisher by that project, each with Publisher's layout export in truth/.
#   fixtures/dev22/      the spike's 22 public test files: 21 from Apache POI's test-data/publisher
#                        (Apache-2.0) and one from LibreOffice's libmspub tests (MPL-2.0), at the pins below.
#                        No layout exports, so the gate checks them against Pubwork's reader.
#   fixtures/lobz/       33 real-world files people attached to LibreOffice import bugs
#                        (bugs.documentfoundation.org) so that importers would get them right, by
#                        attachment id. They can carry real names, so they are never committed.
#   fixtures/pubwork/    Pubwork's patched libmspub WebAssembly reader (Daffyllas/Pubwork, MPL-2.0): the
#                        gate's second reference (gate/pubwork/oracle.mjs runs it under Node).
#
# pubsmith is MIT, but its THIRD-PARTY-NOTICES.md says these files carry Microsoft material outside that
# licence, so we fetch them for local testing and never vendor them. Their truth JSON scores imports
# only; it is never used to work out the file format.
#
# Then: cargo xtask gate --set pubsmith --names fixtures/pubsmith
#       cargo xtask gate --set dev22 --names fixtures/dev22
#       cargo xtask gate --set lobz --names fixtures/lobz
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

echo "==> lobz (LibreOffice Bugzilla attachments)"
mkdir -p "$ROOT/fixtures/lobz"
while read -r bug att sha; do
  fetch "https://bugs.documentfoundation.org/attachment.cgi?id=$att" "$ROOT/fixtures/lobz/lo-bz$bug-$att.pub" "$sha"
done <<'LIST'
60556 74547 c6282468f39770d0dfa6f0724c0c4b86d53913bd41350992498fd732ed4d2439
61579 75659 518c2991eb77403381b9fb1f7236cb0f22fb6bc453da3c2244222eeb4b2f57bd
64631 79338 1c4ec15176519b7ac98e63fc1f038db9c4575ef69cb59525735ec3aebf312294
67299 83079 53306e7e76683b5332dda5be30c4b669ae23abcd13345c954356d101f2b30177
68259 84229 e2419e6e08efc1fa70ee8b9941ea3e80232d4042ccaeec4cd9f72a065ede68df
68259 84230 8fc593ba08060737eb7eee7d61e45d8d11ff68adf3c4e7de4acb3951abaaabfe
68259 84231 d24ccb4ea3cf298c76bf9e18c3202c9b7f613288ca2c9a1b28161253d4406d4c
68259 84232 593640c5168d595a6dd94af1d6e483d590f0582de3fa1eeb1c8baeb7d7553d8f
68259 84568 e12c721d89482d163af0b837ebe3553c64ad17ad3239a8b7f346c8e6d9951bf8
69080 99486 c828c93282251f9cf3040753b4ac0ee767cc8b15a88e09c61872da69f6239f2d
69080 131925 facb8dd2330ff20105cea3bb9ac3b0ff8c1017ffb84ff3f3c4abdb8fb392c100
78733 99073 9645e2831ab7e5a6676a312c0992547182475100cf32ad20740d24532d29973e
78739 99784 e74043b2606ce5d479de95710ec7925baa4710ad587ef3647e4fcda0e802614a
79182 99721 9a0c19bba8d2e52448e2096dca74e6f546bcf5241b4afb63cc61f114809d9d2e
80661 101957 e090e43a8795922a2bd51652a6404c89df89f68394266e6991fe94a0e40161b4
84096 106549 b298136dd5571a7a4e6074589df54a5342c44a5e69d20b57270b835f3e9a92b7
89061 113068 31df4b54e17815c2b8b7aa8da216f36b19dc56c833c0e321a72a8bb279026e69
89393 113398 3b86147ce740920b663b98249aa47c1fda26f49db870c1d23a8cd2447217db15
89993 114080 8ce39a18c185f2557e50c04df1fb7355a9aee4a9a7eb481a2c582259e20edae9
92040 116489 a59b2b22b99a607d11f5391587217fe0397b5d667ce1afbbd16cf6485eccb6f8
95348 120057 df4b44ddc1b2d3ce255d49c7cfcdeff9c56ac3ca216890017e1011c72985f76f
95894 120611 a68ed9e7a4a539884e43145e9387fcc0df40b15ad6d9d70ea5ede8b941729fbd
100639 125942 bc7a640906d22b5c08f13a3c0b175591ed47239095c0ddcb5d412eaa376e0fdb
102039 127356 59589a33472760953c8bd873eedf6e279015175e108292e488585798bf71ce71
105567 130715 e29aff93834107d0d74b1f04312934042419335a245d04d8432f84f9caf50410
116018 140164 e8cfe6bdc421d136e3881b624dcb9e5bd1d170c4ab54de381bfed46da17a0372
121800 147164 b619a8f92d4e522d1d0969451dd75dec5477a2f7af97d311a4a089a5c2273087
129090 156177 4ef8a91d781e26967fa2a67de14f3f203af0d2792afc467097239b4b44d6ff1f
130289 157539 d351e31d1064167992375f5e9e0c5f065a1a3783a41bc3e24f43b7b7f5353411
133297 161173 8e683113ecb18ce1cac132a083a81835870b84aef81f50971e77f4161c9a9028
150838 182274 741aa54ac67ef5c1c35547395919b761631428b9973a2e40f07bf54ccfa419ff
160489 193443 7156f8ebf1ef75ea870803dd5a702fd142b86400afe7dcbdced79aa30e1af2c8
163388 197010 e07f44af250244ffc4a2e5abf35e9ee74493bdb38a60b6cc060c1bdfe6195919
LIST
echo "==> $(find "$ROOT/fixtures/lobz" -name '*.pub' | wc -l | tr -d ' ') .pub files in $ROOT/fixtures/lobz"

echo "==> Pubwork's reader at ${PUBWORK_COMMIT:0:12}"
mkdir -p "$ROOT/fixtures/pubwork"
PW="https://raw.githubusercontent.com/Daffyllas/Pubwork/$PUBWORK_COMMIT"
fetch "$PW/public/pubreader/pubreader.mjs" "$ROOT/fixtures/pubwork/pubreader.mjs" 48830873c9d8141e9da679e1847010111077f01150835acf51e31f8f1f63a0c6
fetch "$PW/public/pubreader/pubreader.wasm" "$ROOT/fixtures/pubwork/pubreader.wasm" 39f541c1cd7a53f669d72f8c6ec113170de2a90a3b75c0ef03d1fdddee0dfc56
fetch "$PW/src/pubmodel.js" "$ROOT/fixtures/pubwork/pubmodel.js" e30e05e3db6d37f5feb1125164e27af8d376c05c158558b384ded5b93edbfbd4
fetch "$PW/LICENSE" "$ROOT/fixtures/pubwork/LICENSE" fab3dd6bdab226f1c08630b1dd917e11fcb4ec5e1e020e2c16f83a0a13863e85
echo "==> Pubwork reader in $ROOT/fixtures/pubwork"
