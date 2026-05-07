# OneBerry DevKit

面向 OneBerry 开发团队的桌面开发环境工具。基于 **Tauri 2 + Vue 3 + TypeScript** 构建，提供 VPN 连接、K8s 集群管理和流量拦截等功能。

## 技术栈

- **桌面框架**: Tauri 2 (Rust)
- **前端**: Vue 3 + TypeScript + Vite
- **UI 组件库**: Naive UI
- **状态管理**: Pinia
- **图标**: Lucide

## 本地开发

### 前提条件

- Node.js 20+
- Rust stable
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)

### 启动

```bash
# 安装依赖
npm install

# 启动开发模式 (前端 + Tauri)
npm run tauri dev
```

前端单独运行（浏览器 Mock 模式）：

```bash
npm run dev
# 访问 http://localhost:1420
```

### 构建

```bash
npm run tauri build
```

## 项目结构

```
oneberry-devkit/
├── src/                    # Vue 前端
│   ├── App.vue             # 主布局 + 环境状态管理
│   ├── views/              # 页面
│   │   ├── Dashboard.vue   # 流量拦截管理
│   │   ├── Settings.vue    # 配置页
│   │   └── Logs.vue        # 日志查看
│   ├── stores/app.ts       # Pinia 状态 (VPN/集群/会话)
│   └── composables/
│       └── useUpdater.ts   # 自动更新逻辑
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── lib.rs          # 应用入口 + 插件注册
│   │   ├── utils.rs        # 跨平台 CLI 工具
│   │   ├── commands/       # Tauri Commands
│   │   │   ├── vpn.rs      # VPN 管理 (Tailscale)
│   │   │   ├── cluster.rs  # K8s 集群状态
│   │   │   ├── session.rs  # 流量拦截会话 (ktctl)
│   │   │   ├── setup.rs    # 环境初始化
│   │   │   └── dns.rs      # 集群 DNS 配置
│   │   └── services/
│   │       └── health_monitor.rs  # 后台健康检查
│   ├── tauri.conf.json     # Tauri 配置
│   └── resources/          # 打包资源
├── scripts/
│   ├── manual-release.sh   # 手动发布脚本
│   └── download-binaries.sh
├── update.json             # 自动更新清单
└── .github/workflows/
    └── build-desktop.yml   # GitHub Actions 构建
```

## 发布流程

发布采用 **GitLab → GitHub → GitLab** 的跨平台构建链路：

```
打 tag → GitLab CI 镜像到 GitHub → GitHub Actions 构建 → 手动发布脚本 → 客户端自动更新
```

### 1. 打 Tag 触发 CI

```bash
git tag v0.3.0
git push origin v0.3.0
```

> 只有 `v` 开头的 tag 才会触发 CI（由 `.gitlab-ci.yml` 的 `workflow.rules` 控制）。

### 2. GitLab CI 镜像到 GitHub

`mirror-to-github` Job 自动执行：

- 将代码 + tag **force push** 到 GitHub 仓库 (`Maroet/oneberry-devkit`)
- 通过 GitHub API 触发 `build-desktop.yml` workflow

需要的 CI/CD 变量：

| 变量 | 用途 |
|------|------|
| `GITHUB_TOKEN` | GitHub PAT (Contents + Actions 权限) |
| `GITHUB_REPO` | GitHub 仓库路径 |

### 3. GitHub Actions 跨平台构建

`build-desktop.yml` 自动完成：

- **矩阵构建**: macOS arm64 + Windows x64
- **版本同步**: 从 tag 提取版本号，更新 `tauri.conf.json`、`Cargo.toml`、`package.json`
- **构建签名**: 使用 `TAURI_SIGNING_PRIVATE_KEY` 对安装包进行 minisign 签名
- **发布**: 创建 GitHub Release，上传安装包和 `.sig` 签名文件

构建产物：

| 平台 | 安装包 | 签名 |
|------|--------|------|
| macOS arm64 | `*.app.tar.gz` | `*.app.tar.gz.sig` |
| Windows x64 | `*-setup.exe` | `*-setup.exe.sig` |

GitHub Secrets 需要配置：

| Secret | 用途 |
|--------|------|
| `TAURI_SIGNING_PRIVATE_KEY` | minisign 私钥，用于签名 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码 |

### 4. 手动发布

> ⚠️ 此步骤需要**人工介入**，在 GitHub Actions 构建完成后本地执行。

```bash
# 配置环境变量 (首次需要)
cp .env.example .env
# 编辑 .env 填写 GITHUB_TOKEN 和 GITLAB_API_TOKEN

# 执行发布
./scripts/manual-release.sh v0.3.0
```

脚本依次完成：

1. 从 GitHub Release **下载**构建产物到 `release-assets/`
2. **上传**到 GitLab Package Registry
3. **生成** `update.json`（包含版本号、下载 URL、签名）
4. **推送** `update.json` 到 main 分支

如果已手动下载产物到 `release-assets/`，可跳过下载：

```bash
SKIP_DOWNLOAD=1 ./scripts/manual-release.sh v0.3.0
```

## 自动更新机制

### 检测流程

客户端启动后 3 秒，`useUpdater.ts` 请求 GitLab 上的 `update.json`：

```
GET https://gitlab.oneberry.cc:2083/api/v4/projects/67/repository/files/update.json/raw?ref=main
Header: PRIVATE-TOKEN: <GitLab Project Access Token>
```

- 比较 `update.json` 中的版本号与当前版本
- 若有新版本，显示 Toast 通知提示用户
- 用户点击「立即更新」→ 下载安装包 → 验证 minisign 签名 → 安装并重启

### update.json 格式

```json
{
  "version": "0.3.0",
  "notes": "OneBerry DevKit v0.3.0",
  "pub_date": "2026-05-08T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "url": "https://gitlab.oneberry.cc:2083/.../OneBerry.DevKit_0.3.0_x64-setup.exe",
      "signature": "<base64 minisign signature>"
    },
    "darwin-aarch64": {
      "url": "https://gitlab.oneberry.cc:2083/.../OneBerry.DevKit_aarch64.app.tar.gz",
      "signature": "<base64 minisign signature>"
    }
  }
}
```

### 签名验证

- 构建时使用 `TAURI_SIGNING_PRIVATE_KEY` 签名
- 客户端使用 `tauri.conf.json` 中的 `pubkey` 验证
- 签名不匹配时拒绝安装，防止篡改

## 推荐 IDE

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
