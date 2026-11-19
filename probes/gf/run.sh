#!/usr/bin/env bash
# Reproduce the GF probe on Yggdrasil. Run from anywhere:  bash probes/gf/run.sh
# Needs: ssh yggdrasil with passwordless sudo docker, the weksa-gf-probe:1 image
# (build it from Dockerfile here), and ~/weksa-gf-probe/rgl (unzipped
# gf-rgl-20260403.zip from the GF RGL GitHub release). See RESULTS.md "Setup".
# Everything runs inside the image, nice'd, as the invoking user.
# GF is installed only inside the image, never on the host.
set -euo pipefail
cd "$(dirname "$0")"
bash sync.sh
ssh yggdrasil 'bash -s' <<'REMOTE'
set -euo pipefail
cd ~/weksa-gf-probe
D="nice sudo -n docker run --rm --user $(id -u):$(id -g) -e HOME=/tmp -v $PWD:/w -e GF_LIB_PATH=/w/rgl/alltenses:/w/rgl/prelude weksa-gf-probe:1 bash -c"
mkdir -p probes/gf/out
echo "== compile grammar"
$D 'cd /w/probes/gf/grammar && rm -f *.gfo Probe.pgf && time gf -make ProbeEng.gf ProbeJpn.gf >/dev/null && ls -l Probe.pgf && mkdir -p ../out && cp Probe.pgf ../out/'
echo "== gf shell linearization (reference)"
$D 'cd /w/probes/gf/grammar && gf --run < trees.gfs'
echo "== rust build"
$D 'cd /w/probes/gf/rust && time cargo build --release 2>&1 | tail -3 && ls -l target/release/gfprobe'
echo "== rust run"
$D 'cd /w/probes/gf/rust && ./target/release/gfprobe ../out/Probe.pgf ProbeEng ProbeJpn'
REMOTE
