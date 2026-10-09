# ---- 构建阶段:下载字体 + 编译 ----
FROM rust:1.98-slim AS builder
WORKDIR /app
COPY . .
# 内置字体(OFL 开源协议)在构建期下载并校验
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN ./scripts/fetch-fonts.sh
RUN cargo build --release

# ---- 运行阶段 ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/zitie /app/zitie
COPY --from=builder /app/static /app/static
COPY --from=builder /app/assets /app/assets

ENV ZITIE_ADDR=0.0.0.0:8080 ZITIE_DATA_DIR=/data
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["/app/zitie"]
