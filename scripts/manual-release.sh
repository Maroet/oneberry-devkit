#!/usr/bin/env bash
# =============================================================================
# 手动发布 OneBerry DevKit
# =============================================================================
#
# 当 GitHub Actions → GitLab CI 自动同步链路不稳定时，使用此脚本手动完成发布：
#   1. 从 GitHub Release 下载构建产物（或使用本地已下载的文件）
#   2. 上传到 GitLab Package Registry
#   3. 生成 update.json 并推送到仓库，触发客户端自动更新
#
# 用法:
#   ./scripts/manual-release.sh <版本号>
#
# 示例:
#   # 从 GitHub 下载并发布
#   ./scripts/manual-release.sh v0.2.0
#
#   # 使用本地已下载的文件（放在 ./release-assets/ 目录）
#   SKIP_DOWNLOAD=1 ./scripts/manual-release.sh v0.2.0
#
# 前提:
#   - 安装 jq, curl
#   - 设置环境变量或在 .env 中配置:
#       GITHUB_TOKEN        GitHub PAT (下载 release assets 需要)
#       GITLAB_API_TOKEN     GitLab API Token (上传到 Package Registry + push 仓库)
#       GITLAB_DEPLOY_TOKEN  (可选) 用于验证 update.json 可访问性
#
# =============================================================================
set -euo pipefail

# ── 颜色 ──
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}ℹ${NC}  $*"; }
ok()    { echo -e "${GREEN}✅${NC} $*"; }
warn()  { echo -e "${YELLOW}⚠${NC}  $*"; }
fail()  { echo -e "${RED}❌${NC} $*"; exit 1; }

# ── 参数检查 ──
TAG="${1:-}"
if [ -z "$TAG" ]; then
  echo "用法: $0 <版本号>"
  echo "示例: $0 v0.2.0"
  exit 1
fi

VERSION="${TAG#v}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

# ── 加载 .env ──
ENV_FILE="${REPO_ROOT}/.env"
if [ -f "$ENV_FILE" ]; then
  info "加载 $ENV_FILE"
  set -a; source "$ENV_FILE"; set +a
fi

# ── 必须的环境变量 ──
GITHUB_TOKEN="${GITHUB_TOKEN:-}"
GITLAB_API_TOKEN="${GITLAB_API_TOKEN:-}"
GITHUB_REPO="${GITHUB_REPO:-Maroet/oneberry-devkit}"
GITLAB_HOST="${GITLAB_HOST:-https://gitlab.oneberry.cc:2083}"
GITLAB_PROJECT_PATH="${GITLAB_PROJECT_PATH:-hongmei-z/oneberry-devkit}"

# GitLab project ID (通过 API 查询)
get_project_id() {
  local encoded response
  encoded=$(printf '%s' "$GITLAB_PROJECT_PATH" | jq -sRr @uri)
  local api_url="${GITLAB_HOST}/api/v4/projects/${encoded}"
  response=$(curl -s --header "PRIVATE-TOKEN: ${GITLAB_API_TOKEN}" "$api_url")
  local proj_id
  proj_id=$(echo "$response" | jq -r '.id // empty' 2>/dev/null)
  if [ -z "$proj_id" ]; then
    warn "API 请求: $api_url" >&2
    warn "API 响应: $(echo "$response" | head -c 300)" >&2
    echo ""
  else
    echo "$proj_id"
  fi
}

# ── 验证依赖 ──
for cmd in jq curl git; do
  command -v "$cmd" &>/dev/null || fail "需要安装 $cmd"
done

ASSET_DIR="${REPO_ROOT}/release-assets"
mkdir -p "$ASSET_DIR"

# =============================================================================
# Step 1: 从 GitHub Release 下载产物
# =============================================================================
if [ "${SKIP_DOWNLOAD:-}" != "1" ]; then
  [ -z "$GITHUB_TOKEN" ] && fail "请设置 GITHUB_TOKEN 环境变量"

  info "从 GitHub Release 下载 ${TAG} 产物..."
  RELEASE_JSON=$(curl -sf \
    -H "Authorization: token ${GITHUB_TOKEN}" \
    "https://api.github.com/repos/${GITHUB_REPO}/releases/tags/${TAG}" \
  ) || fail "未找到 GitHub Release: ${TAG}"

  # 提取需要下载的文件列表到临时文件 (避免 pipe subshell 问题)
  ASSET_LIST=$(mktemp)
  echo "$RELEASE_JSON" | jq -r '.assets[] | "\(.id)|\(.name)|\(.size)"' > "$ASSET_LIST"

  DOWNLOAD_COUNT=0
  while IFS='|' read -r asset_id name size; do
    # 只下载 updater 需要的文件
    case "$name" in
      *.app.tar.gz|*.app.tar.gz.sig|*-setup.exe|*-setup.exe.sig) ;;
      *) continue ;;
    esac

    size_mb=$(echo "scale=1; $size / 1048576" | bc 2>/dev/null || echo "?")

    if [ -f "${ASSET_DIR}/${name}" ]; then
      info "  跳过 (已存在): ${name} (${size_mb}MB)"
      continue
    fi

    info "  ↓ 下载: ${name} (${size_mb}MB)"
    curl -fL# \
      -H "Authorization: token ${GITHUB_TOKEN}" \
      -H "Accept: application/octet-stream" \
      -o "${ASSET_DIR}/${name}" \
      "https://api.github.com/repos/${GITHUB_REPO}/releases/assets/${asset_id}" \
      || fail "下载失败: ${name}"
    ok "    完成: $(ls -lh "${ASSET_DIR}/${name}" | awk '{print $5}')"
    DOWNLOAD_COUNT=$((DOWNLOAD_COUNT + 1))
  done < "$ASSET_LIST"
  rm -f "$ASSET_LIST"

  ok "产物下载完成 (${DOWNLOAD_COUNT} 个文件)"
else
  info "跳过下载，使用 ${ASSET_DIR} 中的本地文件"
fi

# ── 检查产物 ──
echo ""
info "产物列表:"
ls -lh "${ASSET_DIR}/"
echo ""

FILE_COUNT=$(find "$ASSET_DIR" -maxdepth 1 -type f | wc -l | tr -d ' ')
[ "$FILE_COUNT" -eq 0 ] && fail "release-assets/ 目录为空，请先下载或手动放入构建产物"

# =============================================================================
# Step 2: 上传到 GitLab Package Registry
# =============================================================================
[ -z "$GITLAB_API_TOKEN" ] && fail "请设置 GITLAB_API_TOKEN 环境变量"

info "查询 GitLab Project ID..."
PROJECT_ID=$(get_project_id)
[ -z "$PROJECT_ID" ] || [ "$PROJECT_ID" = "null" ] && fail "无法获取项目 ID，请检查 GITLAB_PROJECT_PATH"
ok "Project ID: ${PROJECT_ID}"

PKG_BASE="${GITLAB_HOST}/api/v4/projects/${PROJECT_ID}/packages/generic/oneberry-devkit/${TAG}"

info "上传到 GitLab Package Registry..."
for file in "${ASSET_DIR}"/*; do
  [ ! -f "$file" ] && continue
  filename=$(basename "$file")
  encoded_filename=$(printf '%s' "$filename" | jq -sRr @uri)

  info "  ↑ ${filename}"
  http_code=$(curl -s -o /dev/null -w "%{http_code}" \
    --header "PRIVATE-TOKEN: ${GITLAB_API_TOKEN}" \
    --upload-file "$file" \
    "${PKG_BASE}/${encoded_filename}")

  if [ "$http_code" = "201" ] || [ "$http_code" = "200" ]; then
    ok "    → HTTP ${http_code}"
  else
    warn "    → HTTP ${http_code} (可能已存在或其他问题)"
  fi
done
ok "上传完成"

# =============================================================================
# Step 3: 生成 update.json
# =============================================================================
info "生成 update.json..."

# 获取签名文件内容
get_sig_content() {
  local pattern="$1"
  local sig_file
  sig_file=$(find "$ASSET_DIR" -maxdepth 1 -name "$pattern" -type f | head -1)
  if [ -n "$sig_file" ] && [ -f "$sig_file" ]; then
    cat "$sig_file"
  else
    echo ""
  fi
}

# 获取实际文件名
get_asset_name() {
  local pattern="$1"
  local file
  file=$(find "$ASSET_DIR" -maxdepth 1 -name "$pattern" ! -name "*.sig" -type f | head -1)
  [ -n "$file" ] && basename "$file" || echo ""
}

SIG_WIN=$(get_sig_content "*-setup.exe.sig")
SIG_MAC_ARM=$(get_sig_content "*aarch64*.app.tar.gz.sig")

WIN_FILE=$(get_asset_name "*-setup.exe")
MAC_ARM_FILE=$(get_asset_name "*aarch64*.app.tar.gz")

PLATFORMS="{}"

if [ -n "$SIG_WIN" ] && [ -n "$WIN_FILE" ]; then
  ENCODED_WIN=$(printf '%s' "$WIN_FILE" | jq -sRr @uri)
  PLATFORMS=$(echo "$PLATFORMS" | jq \
    --arg url "${PKG_BASE}/${ENCODED_WIN}" \
    --arg sig "$SIG_WIN" \
    '. + {"windows-x86_64": {"url": $url, "signature": $sig}}')
  ok "  windows-x86_64 → ${WIN_FILE}"
fi

if [ -n "$SIG_MAC_ARM" ] && [ -n "$MAC_ARM_FILE" ]; then
  ENCODED_MAC=$(printf '%s' "$MAC_ARM_FILE" | jq -sRr @uri)
  PLATFORMS=$(echo "$PLATFORMS" | jq \
    --arg url "${PKG_BASE}/${ENCODED_MAC}" \
    --arg sig "$SIG_MAC_ARM" \
    '. + {"darwin-aarch64": {"url": $url, "signature": $sig}}')
  ok "  darwin-aarch64 → ${MAC_ARM_FILE}"
fi

PLATFORM_COUNT=$(echo "$PLATFORMS" | jq 'keys | length')
[ "$PLATFORM_COUNT" -eq 0 ] && fail "没有找到任何有效的平台产物 (需要 .sig 签名文件)"

PUB_DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)
UPDATE_JSON=$(jq -n \
  --arg v "$VERSION" \
  --arg d "$PUB_DATE" \
  --argjson p "$PLATFORMS" \
  '{version: $v, notes: ("OneBerry DevKit v" + $v), pub_date: $d, platforms: $p}')

echo ""
info "update.json 内容:"
echo "$UPDATE_JSON" | jq .
echo ""

# =============================================================================
# Step 4: 推送 update.json 到仓库
# =============================================================================
info "推送 update.json 到仓库..."

# 先写入本地仓库
echo "$UPDATE_JSON" > "${REPO_ROOT}/update.json"
ok "update.json 已写入本地"

cd "$REPO_ROOT"
if git diff --quiet update.json 2>/dev/null; then
  info "update.json 无变化，跳过 git 提交"
else
  git add update.json
  git commit -m "release: update update.json for ${TAG}"
  git push origin HEAD:main
  ok "update.json 已推送到 GitLab"
fi

# =============================================================================
# 完成
# =============================================================================
echo ""
echo -e "${GREEN}══════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  ✅ 发布完成: OneBerry DevKit ${TAG}${NC}"
echo -e "${GREEN}══════════════════════════════════════════════════════${NC}"
echo ""
echo "  📦 Package Registry:"
echo "     ${GITLAB_HOST}/${GITLAB_PROJECT_PATH}/-/packages"
echo ""
echo "  📄 update.json 端点:"
echo "     ${GITLAB_HOST}/${GITLAB_PROJECT_PATH}/-/raw/main/update.json"
echo ""
echo "  客户端将在下次启动时自动检查并提示更新。"
echo ""
