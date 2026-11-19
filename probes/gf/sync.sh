#!/usr/bin/env bash
# dev helper: push probes/gf sources to Yggdrasil
cd "$(dirname "$0")" && tar cf - --exclude=dl --exclude=target --exclude=out --exclude=out . | ssh yggdrasil 'mkdir -p ~/weksa-gf-probe/probes/gf && tar xf - -C ~/weksa-gf-probe/probes/gf'
