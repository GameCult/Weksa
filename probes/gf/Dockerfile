FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev cargo rustc libgmp10 libtinfo6 unzip ca-certificates && rm -rf /var/lib/apt/lists/*
COPY dl/gf-3.12-ubuntu-24.04.deb /tmp/gf.deb
RUN dpkg -i /tmp/gf.deb && rm /tmp/gf.deb && ldconfig
