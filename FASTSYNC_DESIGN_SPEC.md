# FastSync - 基于 SSH 的增量文件同步工具

## 技术设计规范文档

**版本**: 1.0  
**日期**: 2026-01-08  
**状态**: 待开发

---

## 1. 项目概述

### 1.1 背景

在开发部署场景中，需要频繁将本地开发的代码同步到远程 Windows/Linux 服务器。传统的全量复制方式效率低下，而现有的 rsync 工具在 Windows 平台上需要额外安装 Cygwin 或 cwRsync，部署不够便捷。

### 1.2 目标

开发一个基于 Rust 的跨平台增量文件同步工具 **FastSync**，具备以下特性：

- **跨平台**: 支持 Windows、Linux、macOS，编译为单一可执行文件
- **增量同步**: 仅传输变化的文件内容，减少网络开销
- **基于 SSH**: 利用现有的 SSH 基础设施，无需额外配置服务端
- **零依赖**: 目标机器无需预装任何软件（agent 模式可选）
- **高性能**: 利用 Rust 的高效内存管理和并发能力

### 1.3 核心使用场景

```bash
# 将本地项目同步到远程服务器
fastsync ./project user@192.168.1.100:/deploy/app

# 指定排除规则
fastsync ./project user@server:/deploy \
    --exclude "node_modules" \
    --exclude ".git"

# 预览模式（不实际传输）
fastsync ./project user@server:/deploy --dry-run
```

---

## 2. 系统架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                           FastSync                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────┐                    ┌─────────────────┐         │
│  │   CLI Module    │                    │  Config Module  │         │
│  │   (clap)        │                    │  (配置解析)      │         │
│  └────────┬────────┘                    └────────┬────────┘         │
│           │                                      │                   │
│           ▼                                      ▼                   │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │                    Core Engine                            │       │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │       │
│  │  │ Scanner  │  │  Delta   │  │ Transfer │  │  Apply   │ │       │
│  │  │ 文件扫描  │  │ 差异计算  │  │ 传输管理  │  │ 应用变更  │ │       │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘ │       │
│  └──────────────────────────────────────────────────────────┘       │
│           │                                                          │
│           ▼                                                          │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │                   Transport Layer                         │       │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │       │
│  │  │  SSH Client  │  │  SCP/SFTP    │  │  Compression │   │       │
│  │  │  (ssh2/russh)│  │  传输        │  │  (zstd)      │   │       │
│  │  └──────────────┘  └──────────────┘  └──────────────┘   │       │
│  └──────────────────────────────────────────────────────────┘       │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘

          │                                    ▲
          │ SSH Connection                     │
          ▼                                    │
┌─────────────────────────────────────────────────────────────────────┐
│                      Remote Server                                   │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │  Mode A: Agent-less (通过 SSH 命令执行)                  │        │
│  │  - 远程执行 find/stat 命令获取文件信息                    │        │
│  │  - 通过 cat/tee 读写文件                                 │        │
│  └─────────────────────────────────────────────────────────┘        │
│                              或                                      │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │  Mode B: Agent (可选，更高效)                            │        │
│  │  - 预部署 fastsync-agent 二进制                          │        │
│  │  - 支持块级签名计算和增量应用                             │        │
│  └─────────────────────────────────────────────────────────┘        │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 运行模式

| 模式 | 描述 | 适用场景 |
|------|------|----------|
| **Agent-less** | 仅依赖远程 SSH，通过标准命令获取文件信息 | 快速部署，无需预装 |
| **Agent** | 在远程部署 `fastsync-agent`，支持块级增量 | 大文件频繁更新场景 |

---

## 3. 核心算法设计

### 3.1 增量同步算法概述

FastSync 采用分层增量策略：

```
Level 1: 文件级增量（默认）
  - 对比文件元数据（路径、大小、修改时间）
  - 只传输变化的文件（整文件）

Level 2: 块级增量（Agent 模式）
  - 采用 rsync 滚动校验和算法
  - 只传输文件中变化的数据块
```

### 3.2 Level 1: 文件级增量算法

#### 3.2.1 流程图

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  扫描本地目录  │────▶│  获取远程清单  │────▶│   对比差异    │
└──────────────┘     └──────────────┘     └──────────────┘
                                                  │
                     ┌────────────────────────────┼────────────────────────────┐
                     ▼                            ▼                            ▼
              ┌──────────────┐           ┌──────────────┐           ┌──────────────┐
              │   新增文件    │           │   修改文件    │           │   删除文件    │
              │  (Upload)    │           │  (Update)    │           │ (可选 Delete)│
              └──────────────┘           └──────────────┘           └──────────────┘
```

#### 3.2.2 文件清单数据结构

```rust
/// 文件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// 相对路径 (使用 / 作为分隔符)
    pub path: String,
    /// 文件大小 (bytes)
    pub size: u64,
    /// 修改时间 (Unix timestamp, seconds)
    pub mtime: i64,
    /// 文件权限 (Unix mode, 如 0o644)
    pub mode: u32,
    /// 是否为目录
    pub is_dir: bool,
}

/// 目录清单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// 清单生成时间
    pub generated_at: i64,
    /// 根目录路径
    pub root_path: String,
    /// 文件列表
    pub entries: Vec<FileEntry>,
}
```

#### 3.2.3 差异检测规则

| 情况 | 本地 | 远程 | 判断结果 |
|------|------|------|----------|
| 新增 | 存在 | 不存在 | 需要上传 |
| 删除 | 不存在 | 存在 | 需要删除（可选） |
| 修改 | size 不同 | - | 需要更新 |
| 修改 | mtime 更新 | - | 需要更新 |
| 相同 | size 相同且 mtime ≤ 远程 | - | 跳过 |

### 3.3 Level 2: 块级增量算法（rsync 核心）

#### 3.3.1 算法原理

rsync 算法的核心是**滚动校验和 (Rolling Checksum)**，能够在 O(n) 时间内找到文件中匹配的数据块。

**算法步骤：**

1. **远程端**：将目标文件按固定大小（如 4KB）分块，计算每块的签名
   - 弱校验和：Adler-32（32位，快速滚动计算）
   - 强校验和：BLAKE3（256位，防碰撞）

2. **本地端**：使用滚动窗口扫描源文件
   - 对每个位置计算弱校验和
   - 若弱校验匹配，再验证强校验
   - 匹配则记录"复制远程块"，否则记录"新数据"

3. **生成 Delta**：一系列指令（复制块 / 新数据）

4. **远程端**：根据 Delta 和原文件重建新文件

#### 3.3.2 数据结构定义

```rust
/// 块大小 (可配置，默认 4KB)
pub const DEFAULT_BLOCK_SIZE: usize = 4096;

/// 块签名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSignature {
    /// 块索引
    pub index: u32,
    /// 弱校验和 (Adler-32)
    pub weak: u32,
    /// 强校验和 (BLAKE3, 前 16 字节足够)
    pub strong: [u8; 16],
}

/// 文件签名（所有块的签名集合）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSignature {
    /// 原始文件大小
    pub file_size: u64,
    /// 块大小
    pub block_size: usize,
    /// 所有块的签名
    pub blocks: Vec<BlockSignature>,
}

/// Delta 指令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOp {
    /// 复制远程文件的指定块
    CopyBlock { index: u32 },
    /// 插入新数据
    Insert { data: Vec<u8> },
}

/// 文件 Delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDelta {
    /// 目标文件路径
    pub path: String,
    /// Delta 指令序列
    pub ops: Vec<DeltaOp>,
    /// 新文件总大小
    pub new_size: u64,
}
```

#### 3.3.3 滚动校验和实现

```rust
/// Adler-32 滚动校验和
pub struct RollingChecksum {
    a: u32,  // sum of bytes
    b: u32,  // weighted sum
    window_size: usize,
}

impl RollingChecksum {
    pub fn new() -> Self {
        Self { a: 1, b: 0, window_size: 0 }
    }

    /// 初始化：计算初始窗口的校验和
    pub fn init(&mut self, data: &[u8]) {
        self.a = 1;
        self.b = 0;
        for (i, &byte) in data.iter().enumerate() {
            self.a = self.a.wrapping_add(byte as u32);
            self.b = self.b.wrapping_add(self.a);
        }
        self.window_size = data.len();
    }

    /// 滚动：移除一个字节，添加一个字节
    pub fn roll(&mut self, old_byte: u8, new_byte: u8) {
        self.a = self.a.wrapping_sub(old_byte as u32).wrapping_add(new_byte as u32);
        self.b = self.b.wrapping_sub((self.window_size as u32).wrapping_mul(old_byte as u32))
                       .wrapping_add(self.a)
                       .wrapping_sub(1);
    }

    /// 获取当前校验和
    pub fn checksum(&self) -> u32 {
        (self.b << 16) | (self.a & 0xffff)
    }
}
```

#### 3.3.4 Delta 计算核心逻辑

```rust
/// 计算 Delta
pub fn compute_delta(local_data: &[u8], remote_sig: &FileSignature) -> FileDelta {
    let block_size = remote_sig.block_size;
    
    // 构建弱校验和 -> 块列表的查找表
    let weak_map: HashMap<u32, Vec<&BlockSignature>> = 
        remote_sig.blocks.iter()
            .fold(HashMap::new(), |mut m, sig| {
                m.entry(sig.weak).or_default().push(sig);
                m
            });
    
    let mut ops = Vec::new();
    let mut pos = 0;
    let mut pending_data = Vec::new();
    let mut rolling = RollingChecksum::new();
    
    if local_data.len() >= block_size {
        rolling.init(&local_data[0..block_size]);
    }
    
    while pos + block_size <= local_data.len() {
        let weak = rolling.checksum();
        
        // 尝试匹配
        let matched = weak_map.get(&weak).and_then(|candidates| {
            let strong = blake3::hash(&local_data[pos..pos+block_size]);
            let strong_prefix: [u8; 16] = strong.as_bytes()[0..16].try_into().unwrap();
            candidates.iter().find(|s| s.strong == strong_prefix)
        });
        
        if let Some(block) = matched {
            // 先输出累积的新数据
            if !pending_data.is_empty() {
                ops.push(DeltaOp::Insert { data: std::mem::take(&mut pending_data) });
            }
            // 输出复制指令
            ops.push(DeltaOp::CopyBlock { index: block.index });
            pos += block_size;
            
            // 重新初始化滚动校验
            if pos + block_size <= local_data.len() {
                rolling.init(&local_data[pos..pos+block_size]);
            }
        } else {
            // 不匹配，记录当前字节为新数据
            pending_data.push(local_data[pos]);
            
            // 滚动窗口
            if pos + block_size < local_data.len() {
                rolling.roll(local_data[pos], local_data[pos + block_size]);
            }
            pos += 1;
        }
    }
    
    // 处理剩余数据
    pending_data.extend_from_slice(&local_data[pos..]);
    if !pending_data.is_empty() {
        ops.push(DeltaOp::Insert { data: pending_data });
    }
    
    FileDelta {
        path: String::new(),  // 由调用者填充
        ops,
        new_size: local_data.len() as u64,
    }
}
```

---

## 4. 模块设计

### 4.1 模块划分

```
fastsync/
├── src/
│   ├── main.rs              # 入口和 CLI 处理
│   ├── lib.rs               # 库入口
│   ├── config.rs            # 配置管理
│   ├── scanner/             # 文件扫描模块
│   │   ├── mod.rs
│   │   ├── local.rs         # 本地文件扫描
│   │   └── filter.rs        # 过滤规则（.gitignore 风格）
│   ├── delta/               # 差异计算模块
│   │   ├── mod.rs
│   │   ├── file_level.rs    # 文件级差异
│   │   ├── block_level.rs   # 块级差异（rsync 算法）
│   │   └── rolling.rs       # 滚动校验和
│   ├── transport/           # 传输模块
│   │   ├── mod.rs
│   │   ├── ssh.rs           # SSH 连接管理
│   │   ├── sftp.rs          # SFTP 传输
│   │   └── channel.rs       # SSH Channel 通信
│   ├── remote/              # 远程操作模块
│   │   ├── mod.rs
│   │   ├── agentless.rs     # 无 Agent 模式
│   │   └── agent.rs         # Agent 模式协议
│   ├── apply/               # 应用变更模块
│   │   └── mod.rs           # Delta 应用逻辑
│   └── util/                # 工具模块
│       ├── mod.rs
│       ├── hash.rs          # 哈希计算
│       └── progress.rs      # 进度显示
├── fastsync-agent/          # Agent 子项目（可选）
│   └── src/
│       └── main.rs
├── Cargo.toml
└── README.md
```

### 4.2 核心接口定义

#### 4.2.1 Scanner 接口

```rust
/// 文件扫描器 trait
pub trait Scanner {
    /// 扫描目录，返回文件清单
    fn scan(&self, path: &Path) -> Result<Manifest>;
}

/// 本地文件扫描器
pub struct LocalScanner {
    /// 排除规则
    exclude_patterns: Vec<Pattern>,
    /// 是否跟随符号链接
    follow_symlinks: bool,
}

impl Scanner for LocalScanner {
    fn scan(&self, path: &Path) -> Result<Manifest>;
}
```

#### 4.2.2 Transport 接口

```rust
/// SSH 连接配置
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    pub timeout: Duration,
}

pub enum SshAuth {
    /// 密钥认证
    PublicKey { key_path: PathBuf, passphrase: Option<String> },
    /// 密码认证
    Password(String),
    /// SSH Agent
    Agent,
}

/// SSH 连接
pub struct SshConnection {
    session: Session,
}

impl SshConnection {
    /// 建立连接
    pub fn connect(config: &SshConfig) -> Result<Self>;
    
    /// 执行远程命令
    pub fn exec(&self, command: &str) -> Result<ExecOutput>;
    
    /// 获取 SFTP 通道
    pub fn sftp(&self) -> Result<Sftp>;
    
    /// 上传文件
    pub fn upload_file(&self, local: &Path, remote: &Path) -> Result<()>;
    
    /// 上传 Delta
    pub fn upload_delta(&self, delta: &FileDelta, base_path: &Path) -> Result<()>;
}
```

#### 4.2.3 Sync Engine 接口

```rust
/// 同步选项
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// 是否删除远程多余文件
    pub delete: bool,
    /// 是否为预览模式
    pub dry_run: bool,
    /// 使用块级增量（需要 Agent）
    pub block_level: bool,
    /// 排除规则
    pub excludes: Vec<String>,
    /// 并发传输数
    pub parallel: usize,
    /// 是否压缩传输
    pub compress: bool,
}

/// 同步引擎
pub struct SyncEngine {
    conn: SshConnection,
    options: SyncOptions,
}

impl SyncEngine {
    pub fn new(conn: SshConnection, options: SyncOptions) -> Self;
    
    /// 执行同步
    pub fn sync(&self, local_path: &Path, remote_path: &Path) -> Result<SyncReport>;
}

/// 同步报告
pub struct SyncReport {
    pub files_scanned: usize,
    pub files_transferred: usize,
    pub bytes_transferred: u64,
    pub files_deleted: usize,
    pub duration: Duration,
    pub errors: Vec<SyncError>,
}
```

---

## 5. 通信协议

### 5.1 Agent 模式协议

Agent 模式下，客户端与远程 Agent 通过 SSH Channel 进行二进制协议通信。

#### 5.1.1 消息格式

```
+--------+--------+----------------+
| Magic  | Length | Payload        |
| 4 bytes| 4 bytes| Variable       |
+--------+--------+----------------+

Magic: 0x46535943 ("FSYC")
Length: Payload 长度 (Big Endian)
Payload: 使用 bincode 序列化的消息体
```

#### 5.1.2 消息类型

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    /// 获取目录清单
    GetManifest { path: String },
    
    /// 获取文件签名（用于块级增量）
    GetSignature { path: String, block_size: usize },
    
    /// 上传完整文件
    UploadFile { path: String, data: Vec<u8>, mode: u32 },
    
    /// 应用 Delta
    ApplyDelta { path: String, delta: FileDelta },
    
    /// 删除文件/目录
    Delete { path: String },
    
    /// 创建目录
    MakeDir { path: String, mode: u32 },
    
    /// 心跳
    Ping,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    /// 操作成功
    Ok,
    
    /// 返回清单
    Manifest(Manifest),
    
    /// 返回签名
    Signature(FileSignature),
    
    /// 错误
    Error { code: i32, message: String },
    
    /// Pong
    Pong,
}
```

---

## 6. CLI 设计

### 6.1 命令格式

```bash
fastsync [OPTIONS] <SOURCE> <DESTINATION>
fastsync [OPTIONS] --server --path <PATH>  # Agent 模式
```

### 6.2 参数说明

| 参数 | 短形式 | 说明 | 默认值 |
|------|--------|------|--------|
| `--exclude` | `-e` | 排除模式（可多次指定） | - |
| `--delete` | - | 删除远程多余文件 | false |
| `--dry-run` | `-n` | 预览模式，不实际传输 | false |
| `--progress` | `-P` | 显示传输进度 | true |
| `--compress` | `-z` | 压缩传输 | false |
| `--parallel` | `-j` | 并发传输数 | 4 |
| `--identity` | `-i` | SSH 私钥路径 | ~/.ssh/id_rsa |
| `--port` | `-p` | SSH 端口 | 22 |
| `--quiet` | `-q` | 安静模式 | false |
| `--verbose` | `-v` | 详细输出 | false |
| `--block-level` | `-b` | 启用块级增量（需 Agent） | false |
| `--checksum` | `-c` | 使用校验和而非时间戳 | false |

### 6.3 使用示例

```bash
# 基本同步
fastsync ./project user@server:/deploy/app

# 多个排除规则
fastsync ./project user@server:/deploy \
    -e "node_modules" \
    -e ".git" \
    -e "*.log" \
    -e "tmp/"

# 完整同步（删除远程多余文件）
fastsync ./project user@server:/deploy --delete

# 使用指定密钥
fastsync ./project user@server:/deploy -i ~/.ssh/deploy_key

# 预览模式
fastsync ./project user@server:/deploy --dry-run

# 块级增量（需要远程 Agent）
fastsync ./project user@server:/deploy --block-level
```

---

## 7. 错误处理

### 7.1 错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum FastSyncError {
    #[error("SSH connection failed: {0}")]
    SshConnection(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
    
    #[error("Remote command failed: {0}")]
    RemoteCommand(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Checksum mismatch for {path}")]
    ChecksumMismatch { path: PathBuf },
}
```

### 7.2 重试策略

- SSH 连接失败：最多重试 3 次，间隔 1/2/4 秒
- 文件传输失败：最多重试 2 次
- 网络中断：自动重连并恢复传输

---

## 8. 性能优化

### 8.1 优化策略

| 策略 | 说明 |
|------|------|
| **并发扫描** | 使用 rayon 并行扫描本地目录 |
| **流式传输** | 大文件采用流式传输，不全部加载到内存 |
| **压缩传输** | 使用 zstd 压缩，平衡压缩率和速度 |
| **连接复用** | 复用 SSH 连接，避免重复握手 |
| **批量操作** | 小文件合并为一个请求处理 |
| **增量扫描** | 可选：记录上次同步状态，只扫描变化部分 |

### 8.2 内存限制

- 块级增量时，签名表大小限制为 100MB
- 大文件流式处理，内存占用 ≤ 2 × block_size

---

## 9. 安全考虑

### 9.1 传输安全

- 所有数据通过 SSH 加密传输
- 支持 SSH Agent 转发，避免密钥明文存储

### 9.2 权限控制

- 遵循远程文件系统权限
- 不会覆盖权限不足的文件

### 9.3 路径安全

- 禁止 `..` 路径遍历
- 远程路径必须是绝对路径

---

## 10. 开发计划

### 10.1 里程碑

| 阶段 | 内容 | 预估时间 |
|------|------|----------|
| **M1: MVP** | 文件级增量 + 基本 SSH 传输 | 1 周 |
| **M2: 功能完善** | 排除规则、进度显示、压缩 | 1 周 |
| **M3: 块级增量** | rsync 算法实现 + Agent | 2 周 |
| **M4: 优化打磨** | 性能优化、错误处理、测试 | 1 周 |

### 10.2 M1 (MVP) 详细任务

- [ ] 项目初始化，配置 Cargo.toml
- [ ] 实现 CLI 参数解析
- [ ] 实现本地文件扫描
- [ ] 实现 SSH 连接和远程命令执行
- [ ] 实现远程文件清单获取（agentless）
- [ ] 实现文件级差异检测
- [ ] 实现 SCP 文件上传
- [ ] 基本错误处理
- [ ] 编写集成测试

### 10.3 技术风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Windows 路径处理 | 中 | 统一使用 `/` 分隔符，转换层处理 |
| SSH 库兼容性 | 低 | 优先使用 ssh2 (libssh2)，备选 russh |
| 大文件内存占用 | 中 | 流式处理，限制缓冲区大小 |

---

## 11. 依赖清单

### 11.1 Cargo.toml

```toml
[package]
name = "fastsync"
version = "0.1.0"
edition = "2021"
authors = ["Your Team"]
description = "Fast incremental file sync over SSH"

[dependencies]
# SSH
ssh2 = "0.9"

# CLI
clap = { version = "4.4", features = ["derive"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"

# File system
walkdir = "2.4"
ignore = "0.4"       # gitignore 支持
globset = "0.4"

# Hashing
adler = "1.0"        # 滚动校验
blake3 = "1.5"       # 强校验

# Compression (可选)
zstd = "0.13"

# Progress bar
indicatif = "0.17"
console = "0.15"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Async (可选，用于并发)
tokio = { version = "1", features = ["full"], optional = true }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tempfile = "3.8"
assert_cmd = "2.0"
predicates = "3.0"

[features]
default = []
async = ["tokio"]

[profile.release]
lto = true
codegen-units = 1
strip = true
```

---

## 12. 测试策略

### 12.1 单元测试

- 滚动校验和算法正确性
- Delta 计算和应用
- 路径规范化

### 12.2 集成测试

- 端到端同步测试（本地 Docker 环境模拟）
- 大文件处理测试
- 网络中断恢复测试

### 12.3 性能基准

- 与 rsync 对比同步速度
- 内存占用监控
- 大目录（10万+文件）扫描性能

---

## 附录 A: 参考资料

1. [rsync 算法论文](https://rsync.samba.org/tech_report/)
2. [ssh2-rs 文档](https://docs.rs/ssh2)
3. [Adler-32 算法](https://en.wikipedia.org/wiki/Adler-32)
4. [BLAKE3 哈希](https://github.com/BLAKE3-team/BLAKE3)

---

## 附录 B: 术语表

| 术语 | 说明 |
|------|------|
| **Manifest** | 文件清单，包含目录下所有文件的元数据 |
| **Delta** | 差异数据，描述如何从旧版本构建新版本 |
| **Rolling Checksum** | 滚动校验和，可高效计算滑动窗口的校验值 |
| **Block** | 文件块，固定大小的文件片段 |
| **Agent** | 部署在远程服务器的 fastsync 辅助程序 |
| **Agentless** | 无需预装程序，仅依赖 SSH 命令 |

---

*文档结束*
