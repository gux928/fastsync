# FastSync 🚀

[![Language](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[English](#english) | [中文](#chinese)

---

<a name="english"></a>
## English

**FastSync** is a high-performance, cross-platform incremental file synchronization tool built with Rust. It provides an efficient alternative to `rsync` for heterogeneous environments, specifically optimized for **Linux to Windows** synchronization.

### 🌟 Key Features

*   **Block-Level Incremental Sync**: Implements the rsync rolling checksum algorithm (Adler32 + BLAKE3). Only transfers modified parts of files.
*   **Agent-less Mode**: Works out-of-the-box over standard SFTP. No special software required on the remote side.
*   **Agent Mode**: Achieve maximum speed by running `fastsync --server` on the remote (automatically handled by the client).
*   **Multi-threaded Parallelism**: Parallel file scanning and uploading to saturate your network bandwidth.
*   **Native Windows Support**: No need for Cygwin or WSL. Comes with a Windows installer and automatic PATH configuration.
*   **Self-Update**: Keep your tool up-to-date with a single command: `fastsync --update`.

### 🚀 Quick Start

#### Prerequisites
*   **Windows**: OpenSSH Server must be installed and running. (Built-in on Windows 10/11, enable it via `Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0`).

#### Installation

**Linux**:
Download the `.deb` package from [Releases](https://github.com/gux928/fastsync/releases) and run:
```bash
sudo apt install ./fastsync_x.x.x_amd64.deb
```
Or install via script:
```bash
curl -fsSL https://raw.githubusercontent.com/gux928/fastsync/master/install.sh | sh
```

**Windows**:
Download the `.msi` from Releases and run the installer.

#### Usage

```bash
# Basic sync (File-level)
fastsync ./local_dir user@192.168.1.100:D:/remote_dir

# High-performance sync (Block-level + Progress bar)
fastsync ./dist Administrator@172.21.97.163:D:/www --block-level -P

# Mirror sync (Delete redundant files on remote)
fastsync ./src user@host:/app --delete --block-level
```

---

<a name="chinese"></a>
## 中文

**FastSync** 是一款基于 Rust 开发的高性能、跨平台增量文件同步工具。它为异构环境（特别是 **Linux 到 Windows**）提供了比传统方式更高效、更简单的同步方案。

### 🌟 核心特性

*   **块级增量同步**：实现 Rsync 滚动校验和算法（Adler32 + BLAKE3），仅传输文件中发生变化的部分。
*   **无代理模式**：直接基于标准 SFTP 工作，远程机器无需安装任何软件。
*   **Agent 模式**：通过在远程运行 `fastsync --server` 实现极速增量比对（客户端自动处理）。
*   **并发同步**：支持多线程并行扫描和上传，充分利用多核 CPU 和网络带宽。
*   **原生 Windows 支持**：无需 Cygwin 或 WSL。提供标准安装包，自动配置环境变量。
*   **自助更新**：一条命令即可升级到最新版本：`fastsync --update`。

### 🚀 快速上手

#### 前置要求 (Prerequisites)
*   **Windows 端**：必须开启 **OpenSSH Server** 服务（Win10/11 及 Server 2019+ 已内置，请确保服务已启动并在防火墙放行 22 端口）。

#### 安装

**Linux**:
从 [Releases](https://github.com/gux928/fastsync/releases) 下载 `.deb` 包并执行：
```bash
sudo apt install ./fastsync_x.x.x_amd64.deb
```
或使用一键安装脚本：
```bash
curl -fsSL https://raw.githubusercontent.com/gux928/fastsync/master/install.sh | sh
```

**Windows**:
从 Releases 下载 `.msi` 并运行安装程序。

#### 常用命令

```bash
# 基础同步（文件级增量）
fastsync ./local_dir user@192.168.1.100:D:/remote_dir

# 极速增量同步（开启块级比对 + 显示进度）
fastsync ./dist Administrator@172.21.97.163:D:/www --block-level -P

# 镜像同步（删除远程多余文件）
fastsync ./src user@host:/app --delete --block-level
```

---

## 🛠 Build from Source

Requirements:
*   Rust 1.70+
*   MinGW-w64 (for Windows cross-compilation)
*   NSIS (for Windows installer packaging)

```bash
./build_release.sh
```

## 📄 License

This project is licensed under the [MIT License](LICENSE).
