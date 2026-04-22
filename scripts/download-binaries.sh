#!/usr/bin/env bash
# =============================================================================
# OneBerry DevKit — 下载内嵌工具二进制
# 
# 用法: ./scripts/download-binaries.sh [--all | --kubectl | --ktctl]
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN_DIR="$PROJECT_ROOT/src-tauri/binaries"

KUBECTL_VERSION="v1.32.0"
KTCTL_VERSION="0.3.7"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }

mkdir -p "$BIN_DIR"

download_kubectl() {
  info "下载 kubectl v${KUBECTL_VERSION}..."

  local targets=(
    "darwin/arm64:kubectl-aarch64-apple-darwin"
    "darwin/amd64:kubectl-x86_64-apple-darwin"
    "windows/amd64:kubectl-x86_64-pc-windows-msvc.exe"
  )

  for entry in "${targets[@]}"; do
    local platform="${entry%%:*}"
    local filename="${entry##*:}"
    local target="$BIN_DIR/$filename"

    if [ -f "$target" ]; then
      warn "  → $filename 已存在，跳过"
      continue
    fi

    info "  → 下载 $filename..."
    curl -fSL "https://dl.k8s.io/release/${KUBECTL_VERSION}/bin/${platform}/kubectl$(echo $filename | grep -q '.exe' && echo '.exe' || true)" -o "$target"
    chmod +x "$target" 2>/dev/null || true
  done

  info "kubectl 下载完成 ✓"
}

download_ktctl() {
  info "下载 ktctl v${KTCTL_VERSION}..."
  local base="https://github.com/alibaba/kt-connect/releases/download/v${KTCTL_VERSION}"

  # macOS ARM64
  local f="$BIN_DIR/ktctl-aarch64-apple-darwin"
  if [ ! -f "$f" ]; then
    info "  → macOS ARM64..."
    curl -fSL "${base}/ktctl_${KTCTL_VERSION}_MacOS_arm_64.tar.gz" -o /tmp/ktctl_mac_arm64.tar.gz
    tar -xzf /tmp/ktctl_mac_arm64.tar.gz -C /tmp/
    mv /tmp/ktctl "$f" && chmod +x "$f"
    rm -f /tmp/ktctl_mac_arm64.tar.gz
  else
    warn "  → macOS ARM64 已存在，跳过"
  fi

  # macOS x86_64
  f="$BIN_DIR/ktctl-x86_64-apple-darwin"
  if [ ! -f "$f" ]; then
    info "  → macOS x86_64..."
    curl -fSL "${base}/ktctl_${KTCTL_VERSION}_MacOS_x86_64.tar.gz" -o /tmp/ktctl_mac_amd64.tar.gz
    tar -xzf /tmp/ktctl_mac_amd64.tar.gz -C /tmp/
    mv /tmp/ktctl "$f" && chmod +x "$f"
    rm -f /tmp/ktctl_mac_amd64.tar.gz
  else
    warn "  → macOS x86_64 已存在，跳过"
  fi

  # Windows x86_64
  f="$BIN_DIR/ktctl-x86_64-pc-windows-msvc.exe"
  if [ ! -f "$f" ]; then
    info "  → Windows x86_64..."
    curl -fSL "${base}/ktctl_${KTCTL_VERSION}_Windows_x86_64.zip" -o /tmp/ktctl_win.zip
    unzip -o /tmp/ktctl_win.zip ktctl.exe -d /tmp/
    mv /tmp/ktctl.exe "$f"
    rm -f /tmp/ktctl_win.zip
  else
    warn "  → Windows x86_64 已存在，跳过"
  fi

  info "ktctl 下载完成 ✓"
}

case "${1:-all}" in
  --kubectl) download_kubectl ;;
  --ktctl)   download_ktctl ;;
  --all|all) download_kubectl; echo ""; download_ktctl ;;
  *) echo "用法: $0 [--all | --kubectl | --ktctl]"; exit 1 ;;
esac

echo ""
info "文件列表："
ls -lh "$BIN_DIR/"
