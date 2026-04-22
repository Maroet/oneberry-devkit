# OneBerry DevKit 开发进度与技术方案报告

## 📌 项目背景与目标

为简化 OneBerry 研发团队的本地联调流程，本项目(DevKit)旨在提供一个跨平台桌面客户端，核心功能包含：
1. 一键自动化连接 Tailscale VPN。
2. 自动检测 Kubenetes 集群网络连通性。
3. 可视化介入 `ktctl exchange` 和 `ktctl mesh` 等高级联调能力，免除繁琐的命令行配置。
4. **单包交付**：kubectl / ktctl 内嵌在应用内，研发无需额外下载安装。

---

## 🛠️ 实现技术方案

### 1. 核心技术栈
- **框架：** Tauri 2.0 构建跨平台桌面应用。
- **后端：** Rust (利用 `tokio` 异步运行时处理后台任务，利用 `std::process::Command` 操纵子进程)。
- **前端：** Vue 3 + TypeScript + Naive UI 组件库。
- **状态管理：** Pinia，并且结合 Tauri 事件总线 (`listen`/`emit`) 实时刷新健康检查状态。

### 2. 前后端通信与任务架构
- **系统工具寻址 (Bin Resolver)：** 跨平台统一的 `find_bin` 模块，自动扫描 macOS (`/opt/homebrew/bin`、`/usr/local/bin`、`/Applications/Tailscale.app/Contents/MacOS`) 和 Windows (`Program Files`、`LOCALAPPDATA`) 的常见安装路径。
- **工具内嵌 (Sidecar Binaries)：** kubectl 和 ktctl 通过 Tauri 的 `externalBin` 机制内嵌到应用 Bundle 中，支持 macOS ARM64/x86_64 + Windows x64 三个平台。
- **Health Monitor：** Rust 端维护一个异步心跳协程 `HealthMonitor`，每 5 秒自动运行 `tailscale status` 和 `kubectl cluster-info` 以检查 VPN 及集群可用性，并通过 Tauri 事件机制推送至前端（`health:vpn`, `health:cluster`）。
- **Mock 模式：** 前端自动检测是否运行在 Tauri 环境中，非 Tauri 环境（浏览器）下返回模拟数据，保证 UI 开发体验。
- **客户端视觉语言：** 采用 Glassmorphism 磨砂玻璃效果 + 渐变色 + 微动画，呈现现代化的开发工具质感。

---

## ✅ 已完成的任务

### P0 阶段 - 框架搭建与预研
1. **项目骨架构建：** Tauri 2.0 + Vue 3 模板初始化，Rust 编译通过。
2. **前端界面开发：** Dashboard / SetupWizard / Settings 三个核心页面。
3. **后端命令集：** vpn / cluster / session / setup 四大模块初步实现。

### P1 阶段 - 完善交互 + 单包交付 ✅
1. **find_bin 全面应用：** 已应用到 setup.rs、health_monitor.rs、session.rs 等全部模块。
2. **跨平台工具内嵌：**
   - 创建 `scripts/download-binaries.sh` 自动下载脚本。
   - kubectl v1.32 (macOS ARM64/x86_64 + Windows x64) — 共 167 MB。
   - ktctl v0.3.7 (macOS ARM64/x86_64 + Windows x64) — 共 116 MB。
   - Tauri `externalBin` + Capabilities 权限配置完成。
3. **Tailscale 安装逻辑：** 使用 Tauri `resource_dir` API 定位内嵌 .pkg/.msi 安装包，跨平台安装。
4. **前端 UI 全面美化：**
   - Glassmorphism 磨砂玻璃卡片 + 渐变色 Icon Box。
   - 微动画系统（fadeIn / fadeInUp / slideInRight / stagger）。
   - 底部状态栏（VPN / Cluster 指示灯 + Mock Mode 标识）。
   - 导航栏重构（控制台 / 设置按钮）。
   - 内嵌工具信息面板（Settings 页展示 kubectl/ktctl 内置版本）。
5. **前端健壮性：** Store 层自动 Tauri 检测 + Mock 数据 fallback，浏览器开发无白屏。
6. **session.rs 跨平台：** macOS 使用 sudo，Windows 直接调用 + taskkill 终止进程。

---

## 🚀 下一步任务 (P2 - P3 阶段)

### P2: 权限剥离与会话管理 (Daemon 机制)
由于 `ktctl exchange` 在 macOS 上启动需要 Root 权限，客户端频繁向用户索要 `sudo` 密码体验极差：
- [ ] **设计提权守护进程 (Daemon IPC)**：考虑在首次 Setup 时，将一个具备对应权限的 Rust 服务以 `launchctl` 或 Windows Service 方式安装到系统底层。
- [ ] **IPC 通信**：Tauri 前端 -> Tauri Rust (普通权限) -> Unix Socket/Named Pipe -> Daemon (高级权限) 执行 `ktctl`。
- [ ] **清理策略**：会话在关闭应用或网络断开时，确保进程被完全中止并恢复服务原有调度。
- [ ] **会话实时日志**：前端展示 ktctl 进程的 stdout/stderr 实时输出。

### P3: CI / CD 集成
- [ ] **Rust 编译验证**：在 `tauri dev` 模式下完整编译并验证 sidecar 调用。
- [ ] **CI / CD 集成**：编写 GitLab CI 的多平台构建脚本（macOS + Windows）。
- [ ] **自动签名与公证**：macOS 的 codesign + notarization 流程。
