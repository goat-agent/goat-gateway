FROM node:20-alpine AS web
WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web ./
RUN npm run build

FROM rust:1-alpine AS build
RUN apk add --no-cache binutils musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY --from=web /build/web web
ENV GOAT_SKIP_WEB_BUILD=1
RUN cargo build --release -p goat-gateway \
    && strip target/release/goat-gateway \
    && install -d -o 65534 -g 65534 /data

FROM scratch
COPY --from=build /build/target/release/goat-gateway /goat-gateway
COPY --from=build /data /data
ENV GOAT_DATA_DIR=/data
VOLUME ["/data"]
EXPOSE 8787
USER 65534:65534
ENTRYPOINT ["/goat-gateway"]
