# OneBerry 研发联调环境搭建指南（Linux CLI）

> 本文档面向使用 Linux 桌面/服务器的研发人员，通过命令行完成联调环境搭建。
> 完成本指南后，你将能够通过 VPN 连接到开发集群，并使用 kt-connect 将集群流量劫持到本地进行调试。

---

## 目录

1. [前置要求](#1-前置要求)
2. [安装 Tailscale（VPN 客户端）](#2-安装-tailscale)
3. [登录 VPN 网络](#3-登录-vpn-网络)
4. [配置 kubeconfig](#4-配置-kubeconfig)
5. [安装 kubectl](#5-安装-kubectl)
6. [验证集群连接](#6-验证集群连接)
7. [安装 kt-connect](#7-安装-kt-connect)
8. [使用 kt-connect 联调](#8-使用-kt-connect-联调)
9. [常用命令速查](#9-常用命令速查)
10. [故障排查](#10-故障排查)

---

## 1. 前置要求

- Linux 发行版：Ubuntu 20.04+ / Debian 11+ / CentOS 8+ / Fedora 36+
- 具有 `sudo` 权限
- 已安装 `curl`、`tar`

---

## 2. 安装 Tailscale

Tailscale 是我们使用的 VPN 组网工具，用于将你的开发机接入内部 K8s 集群网络。

### 一键安装（推荐）

```bash
curl -fsSL https://tailscale.com/install.sh | sh
```

### 手动安装（Ubuntu/Debian）

```bash
# 添加 Tailscale 仓库
curl -fsSL https://pkgs.tailscale.com/stable/ubuntu/jammy.noarmor.gpg | sudo tee /usr/share/keyrings/tailscale-archive-keyring.gpg >/dev/null
curl -fsSL https://pkgs.tailscale.com/stable/ubuntu/jammy.tailscale-keyring.list | sudo tee /etc/apt/sources.list.d/tailscale.list

# 安装
sudo apt-get update
sudo apt-get install -y tailscale
```

### 验证安装

```bash
tailscale version
# 预期输出: 1.x.x
```

### 启动 Tailscale 守护进程

```bash
sudo systemctl enable --now tailscaled
```

---

## 3. 登录 VPN 网络

我们使用自建的 Headscale 控制服务器，需要通过 OIDC（GitLab 账号）认证。

```bash
sudo tailscale up --login-server https://vpn.oneberry.cc:31443 --accept-routes --reset
```

执行后终端会输出一个认证 URL，类似：

```
To authenticate, visit:
  https://vpn.oneberry.cc:31443/oidc/...
```

**在浏览器中打开该链接**，使用 GitLab 账号登录完成认证。

### 验证 VPN 连接

```bash
tailscale status
```

预期看到你的设备和其他节点（如 `hmdev-node01`）均已连接。

### 测试集群网络连通性

```bash
# 测试 K8s API Server 是否可达
ping -c 3 192.168.33.6
```

---

## 4. 配置 kubeconfig

kubeconfig 文件包含连接 K8s 集群所需的认证信息。

### 方式一：从 DevKit 仓库获取（推荐）

```bash
# 克隆 devkit 仓库（如果还没有）
git clone <devkit-repo-url> ~/workspace/oneberry-devkit

# 复制 kubeconfig
mkdir -p ~/.kube
cp ~/workspace/oneberry-devkit/src-tauri/resources/kubeconfig ~/.kube/config
chmod 600 ~/.kube/config
```

### 方式二：手动创建

向团队成员获取 kubeconfig 文件，然后：

```bash
mkdir -p ~/.kube
cp /path/to/kubeconfig ~/.kube/config
chmod 600 ~/.kube/config
```

> **⚠️ 安全提醒**：kubeconfig 包含集群管理员凭证，请勿泄露或提交到 Git 仓库。

---

## 5. 安装 kubectl

kubectl 是 Kubernetes 的命令行管理工具。

### 通过官方源安装（推荐）

```bash
# 下载 v1.32.0（与集群版本匹配）
curl -LO "https://dl.k8s.io/release/v1.32.0/bin/linux/amd64/kubectl"

# 安装
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl
rm kubectl
```

### 通过包管理器安装

```bash
# Ubuntu/Debian
sudo apt-get update && sudo apt-get install -y apt-transport-https
curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.32/deb/Release.key | sudo gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg
echo 'deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/v1.32/deb/ /' | sudo tee /etc/apt/sources.list.d/kubernetes.list
sudo apt-get update
sudo apt-get install -y kubectl
```

### 验证

```bash
kubectl version --client --short
# 预期输出: Client Version: v1.32.x
```

---

## 6. 验证集群连接

确保 VPN 已连接后，测试 K8s 集群访问：

```bash
# 查看集群节点
kubectl get nodes

# 查看 oneberry-dev 命名空间的服务
kubectl get pods -n oneberry-dev

# 查看部署列表
kubectl get deployments -n oneberry-dev
```

如果以上命令正常返回结果，说明集群连接成功 ✅

---

## 7. 安装 kt-connect

[kt-connect](https://github.com/alibaba/kt-connect) 是阿里巴巴开源的 Kubernetes 联调工具，支持将集群中的服务流量劫持到本地。

### 下载安装

```bash
# 下载 v0.3.7
KT_VERSION="0.3.7"
curl -fSL "https://github.com/alibaba/kt-connect/releases/download/v${KT_VERSION}/ktctl_${KT_VERSION}_Linux_x86_64.tar.gz" \
  -o /tmp/ktctl.tar.gz

# 解压安装
tar -xzf /tmp/ktctl.tar.gz -C /tmp/
sudo install -o root -g root -m 0755 /tmp/ktctl /usr/local/bin/ktctl
rm /tmp/ktctl.tar.gz /tmp/ktctl
```

### 验证

```bash
ktctl --version
# 预期输出: ktctl version 0.3.7
```

---

## 8. 使用 kt-connect 联调

kt-connect 需要 root 权限（因为需要操作网络 namespace），所有 `ktctl` 命令需要使用 `sudo`。

### 8.1 Exchange 模式（替换模式）

**用途**：将集群中某个服务的流量 **全部** 劫持到本地。适合独占调试场景。

```bash
# 将 oneberry-ai 服务的流量劫持到本地 8080 端口
# 前提：你的本地服务已在 8080 端口启动
sudo ktctl exchange oneberry-ai --expose 8080 -n oneberry-dev
```

> **⚠️ 注意**：Exchange 模式会影响所有访问该服务的请求，其他同事的请求也会被劫持到你本地。使用完毕后务必停止。

### 8.2 Mesh 模式（网格模式，推荐）

**用途**：通过 HTTP Header 路由，仅将带有特定版本标记的请求转发到本地，不影响其他人。

```bash
# 将带 version=test-haoxiang 的请求转发到本地 8080
sudo ktctl mesh oneberry-ai --expose 8080 --versionMark test-haoxiang -n oneberry-dev
```

使用 Mesh 模式时，需要在请求中添加版本 Header：

```bash
# 测试：带 Header 的请求会路由到你本地
curl -H "X-Kt-Version: test-haoxiang" http://oneberry-ai.oneberry-dev.svc:8080/api/test

# 不带 Header 的请求走集群原服务
curl http://oneberry-ai.oneberry-dev.svc:8080/api/test
```

### 8.3 停止联调

直接 `Ctrl+C` 即可停止 ktctl 进程。如果异常退出导致残留：

```bash
# 清理某个服务的残留
sudo ktctl recover oneberry-ai -n oneberry-dev

# 同时清理 K8s 注解（防止新 exchange 被阻塞）
kubectl annotate svc oneberry-ai kt-selector- -n oneberry-dev
```

---

## 9. 常用命令速查

### VPN 管理

```bash
# 查看 VPN 状态
tailscale status

# 连接 VPN
sudo tailscale up --login-server https://vpn.oneberry.cc:31443 --accept-routes --reset

# 断开 VPN
sudo tailscale down

# 查看分配的 IP
tailscale ip
```

### K8s 操作

```bash
# 查看服务列表
kubectl get deployments -n oneberry-dev

# 查看 Pod 状态
kubectl get pods -n oneberry-dev

# 查看 Pod 日志
kubectl logs -f <pod-name> -n oneberry-dev

# 重启某个 Deployment
kubectl rollout restart deployment/<name> -n oneberry-dev
```

### kt-connect 联调

```bash
# Exchange（替换）
sudo ktctl exchange <service> --expose <port> -n oneberry-dev

# Mesh（网格，推荐）
sudo ktctl mesh <service> --expose <port> --versionMark <tag> -n oneberry-dev

# 清理残留
sudo ktctl recover <service> -n oneberry-dev
```

---

## 10. 故障排查

### VPN 连不上

```bash
# 检查 tailscaled 守护进程
sudo systemctl status tailscaled

# 查看详细日志
sudo journalctl -u tailscaled -f

# 重启守护进程
sudo systemctl restart tailscaled
```

### kubectl 连接超时

```bash
# 1. 确认 VPN 已连接
tailscale status

# 2. 测试 API Server 网络可达
curl -k https://192.168.33.6:6443/healthz

# 3. 确认 kubeconfig 正确
kubectl config view
```

### ktctl 启动失败

```bash
# 确认本地服务已在监听
ss -tlnp | grep <port>

# 查看 ktctl 详细日志
sudo ktctl exchange <service> --expose <port> -n oneberry-dev --debug

# 清理之前的残留会话
sudo ktctl recover <service> -n oneberry-dev
```

### DNS 解析问题

Tailscale 启用 MagicDNS 后可能会劫持系统 DNS，导致外网解析异常：

```bash
# 检查当前 DNS 配置
resolvectl status

# 如果外网 DNS 被劫持，禁用 MagicDNS
sudo tailscale up --login-server https://vpn.oneberry.cc:31443 --accept-routes --accept-dns=false --reset
```

---

## 环境信息

| 项目 | 值 |
|------|----|
| Headscale URL | `https://vpn.oneberry.cc:31443` |
| K8s API Server | `https://192.168.33.6:6443` |
| 默认命名空间 | `oneberry-dev` |
| kubectl 版本 | v1.32.0 |
| ktctl 版本 | v0.3.7 |
| Shadow 镜像 | `image.hm.metavarse.tech:9443/hongmei-dev/kt-connect-shadow:v0.3.7` |
