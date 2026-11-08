
FROM lukemathwalker/cargo-chef:latest-rust-trixie@sha256:420b7375dc4059a85f1c365f23f96b8f561c2861e365645b413c531b206c528a AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release

# The Beszel monitoring agent, fetched in its own stage so the runtime image
# gains no curl and no apt lists. The agent is a static musl binary, so it runs
# unchanged on this glibc base. See docker/beszel-entrypoint.sh.
FROM ubuntu:26.04 AS beszel-agent
ARG BESZEL_AGENT_VERSION=0.21.1
# sha256 of beszel-agent_linux_amd64.tar.gz, from the release's own
# beszel_<version>_checksums.txt.
ARG BESZEL_AGENT_SHA256=3b6ee592632ec03b9a43312e99a40203a7b3d83fe40dd0c25acda1eb915ea7c0
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSLo /tmp/agent.tgz \
         "https://github.com/henrygd/beszel/releases/download/v${BESZEL_AGENT_VERSION}/beszel-agent_linux_amd64.tar.gz" \
    && echo "${BESZEL_AGENT_SHA256}  /tmp/agent.tgz" | sha256sum -c - \
    && tar -xzf /tmp/agent.tgz -C /usr/local/bin beszel-agent \
    && rm /tmp/agent.tgz

# We do not need the Rust toolchain to run the binary!
FROM ubuntu:26.04@sha256:6a06111c3b0a8600feadb270aec771dc239bf407b25625f93f44ab00e11f4d3c AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/rs-httpbin /usr/local/bin
COPY --from=beszel-agent /usr/local/bin/beszel-agent /usr/local/bin/beszel-agent
COPY docker/beszel-entrypoint.sh /usr/local/bin/beszel-entrypoint.sh
RUN chmod +x /usr/local/bin/beszel-entrypoint.sh
ENTRYPOINT ["/usr/local/bin/beszel-entrypoint.sh"]
CMD ["/usr/local/bin/rs-httpbin"]
