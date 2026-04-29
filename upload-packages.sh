#!/bin/bash
# ============================================================
# 手动上传 DevKit 安装包到 GitLab Package Registry
#
# 用法:
#   export GITLAB_TOKEN="你的GitLab Personal Access Token"
#   bash upload-packages.sh
# ============================================================

set -euo pipefail

# ---- 配置 ----
GITLAB_URL="https://gitlab.oneberry.cc:2083"
API_URL="${GITLAB_URL}/api/v4"
GROUP="hongmei-z"
# DevKit 安装包上传到 oneberry-devkit 项目
DEVKIT_PROJECT="oneberry-devkit"
# 第三方工具上传到 oneberry-wiki 项目 (公共资源)
TOOLS_PROJECT="oneberry-wiki"
VERSION="0.1.0"

TOKEN="${GITLAB_TOKEN:?请先 export GITLAB_TOKEN=你的token}"

# ---- 查找 Project ID ----
get_project_id() {
  local project_path="${GROUP}/${1}"
  curl -sk --header "PRIVATE-TOKEN: ${TOKEN}" \
    "${API_URL}/projects/$(echo "$project_path" | sed 's/\//%2F/g')" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])"
}

echo "🔍 获取 Project ID..."
DEVKIT_PID=$(get_project_id "$DEVKIT_PROJECT")
TOOLS_PID=$(get_project_id "$TOOLS_PROJECT")
echo "  oneberry-devkit: ${DEVKIT_PID}"
echo "  oneberry-wiki:   ${TOOLS_PID}"

# ---- 上传函数 ----
upload_file() {
  local project_id="$1"
  local package_name="$2"
  local version="$3"
  local filepath="$4"
  local filename=$(basename "$filepath")

  echo "  ↑ ${filename} ($(du -sh "$filepath" | cut -f1))"
  HTTP_CODE=$(curl -sk -o /dev/null -w "%{http_code}" \
    --header "PRIVATE-TOKEN: ${TOKEN}" \
    --upload-file "$filepath" \
    "${API_URL}/projects/${project_id}/packages/generic/${package_name}/${version}/${filename}")
  
  if [ "$HTTP_CODE" = "201" ] || [ "$HTTP_CODE" = "200" ]; then
    echo "    ✅ HTTP ${HTTP_CODE}"
  else
    echo "    ❌ HTTP ${HTTP_CODE}"
  fi
}

# ---- 1. 上传 macOS DevKit ----
echo ""
echo "📦 上传 macOS DevKit..."
DMG_FILE="/Users/wuhaoxiang/workspace/oneberry-devkit/src-tauri/target/release/bundle/dmg/OneBerry DevKit_0.1.0_aarch64.dmg"
if [ -f "$DMG_FILE" ]; then
  upload_file "$DEVKIT_PID" "oneberry-devkit" "$VERSION" "$DMG_FILE"
else
  echo "  ⚠️ DMG 文件不存在: $DMG_FILE"
fi

# ---- 2. 上传 Tailscale Windows 安装包 ----
echo ""
echo "📦 上传 Tailscale Windows 安装包..."
TAILSCALE_FILE="/Users/wuhaoxiang/workspace/oneberry-devkit/src-tauri/target/release/resources/tailscale-windows.msi"
if [ -f "$TAILSCALE_FILE" ]; then
  upload_file "$TOOLS_PID" "third-party-tools" "latest" "$TAILSCALE_FILE"
else
  echo "  ⚠️ Tailscale MSI 文件不存在: $TAILSCALE_FILE"
fi

# ---- 3. Windows DevKit (本地没有，需要 CI 构建) ----
echo ""
echo "📋 Windows DevKit 安装包说明:"
echo "   Windows exe 需要在 Windows 环境构建，无法在 macOS 上交叉编译。"
echo "   方式一: 打 tag 触发 CI → GitHub Actions 自动构建 → 产物回传 Package Registry"
echo "   方式二: 在 Windows 机器上 cargo tauri build → 手动上传"
echo ""

# ---- 完成 ----
echo "📊 完成！查看已上传的包:"
echo "  DevKit:   ${GITLAB_URL}/${GROUP}/${DEVKIT_PROJECT}/-/packages"
echo "  工具:     ${GITLAB_URL}/${GROUP}/${TOOLS_PROJECT}/-/packages"
