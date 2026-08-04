#!/usr/bin/env bash
# Package Eventide release binaries into dist/<PACKAGE_NAME>.tar.gz
set -euo pipefail

PACKAGE_NAME="${PACKAGE_NAME:?PACKAGE_NAME is required}"
PACKAGE_LABEL="${PACKAGE_LABEL:-$PACKAGE_NAME}"
# Native build: target/release ; cross: target/<triple>/release
BIN_DIR="${BIN_DIR:-target/release}"

STAGE="dist/${PACKAGE_NAME}"
rm -rf "${STAGE}"
mkdir -p "${STAGE}/static" "${STAGE}/keys"

cp "${BIN_DIR}/eventide" "${STAGE}/"
cp "${BIN_DIR}/eventide-trap" "${STAGE}/"
cp "${BIN_DIR}/eventide-license" "${STAGE}/"
chmod +x "${STAGE}/eventide" "${STAGE}/eventide-trap" "${STAGE}/eventide-license"

cp -r static/* "${STAGE}/static/"
cp eventide.toml.example "${STAGE}/"
cp eventide-trap.toml.example "${STAGE}/"
cp keys/license_public.pem "${STAGE}/keys/"
cp keys/README.md "${STAGE}/keys/"

{
  echo "Eventide package: ${PACKAGE_LABEL}"
  echo
  echo "Binaries:"
  echo "  eventide           — 主服务（控制台 + API）"
  echo "  eventide-trap      — SNMP Trap 接收服务"
  echo "  eventide-license   — 厂商侧签发 / 校验许可证"
  echo
  echo "Quick start:"
  echo "  1. cp eventide.toml.example eventide.toml"
  echo "  2. cp eventide-trap.toml.example eventide-trap.toml"
  echo "  3. 编辑 MySQL / Redis / Kafka 等连接信息"
  echo "  4. ./eventide eventide.toml"
  echo "  5. ./eventide-trap eventide-trap.toml"
  echo
  echo "控制台静态资源在 ./static/，请与二进制同目录部署（或改 static_dir）。"
} > "${STAGE}/README.txt"

mkdir -p dist
tar -C dist -czf "dist/${PACKAGE_NAME}.tar.gz" "${PACKAGE_NAME}"
ls -lh "dist/${PACKAGE_NAME}.tar.gz"
find "${STAGE}" -type f | sort
