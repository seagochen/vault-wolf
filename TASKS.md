# 项目任务清单

> **项目名称**: vault-wolf-ibapi (Rust 原生 IB API 客户端库)
> **创建日期**: 2026-02-21
> **最后更新**: 2026-02-21

---

## 项目背景

当前 vault-wolf 依赖第三方 `ibapi` crate (v2.7) 与 IB TWS/Gateway 通信。该 crate 功能覆盖有限，且无法自主控制协议细节。本项目目标是参照 `cppclient/` 中的 C++ 官方 API 源码，构建一个 **Rust 原生 IB API 客户端库**，作为独立 crate 供 vault-wolf 及未来项目使用。

### 架构设计方案

```
vault-wolf-ibapi/          ← 新建独立 crate
├── src/
│   ├── lib.rs             # 公共 API 导出
│   ├── client.rs          # IBClient (对应 EClient + EClientSocket)
│   ├── wrapper.rs         # Wrapper trait (对应 EWrapper, 125+ 回调)
│   ├── reader.rs          # MessageReader (对应 EReader, 异步消息读取)
│   ├── decoder.rs         # MessageDecoder (对应 EDecoder, 协议解析)
│   ├── encoder.rs         # 请求编码 (对应 EClient 中的编码逻辑)
│   ├── transport.rs       # TCP 传输层 (对应 ESocket)
│   ├── protocol.rs        # 协议常量、消息 ID、版本号
│   ├── errors.rs          # 错误类型定义
│   ├── models/
│   │   ├── mod.rs
│   │   ├── contract.rs    # Contract, ComboLeg, DeltaNeutralContract
│   │   ├── order.rs       # Order, OrderState, OrderCondition
│   │   ├── execution.rs   # Execution, ExecutionFilter
│   │   ├── bar.rs         # Bar (OHLCV), HistoricalTick
│   │   ├── account.rs     # AccountSummary, Position 相关
│   │   ├── market_data.rs # TickData, DepthMktData, TickAttrib
│   │   ├── scanner.rs     # ScannerSubscription
│   │   └── common.rs      # TagValue, SoftDollarTier, Decimal 等
│   └── constants.rs       # MIN_SERVER_VER_*, 消息 ID 常量
├── Cargo.toml
└── tests/
    ├── connection_test.rs
    ├── encoding_test.rs
    └── integration_test.rs
```

### 技术方案决策

| 决策项 | 选型 | 理由 |
|--------|------|------|
| 异步运行时 | Tokio | 与 vault-wolf 主项目一致 |
| 网络 I/O | tokio::net::TcpStream | 原生异步，替代 C++ 的 POSIX socket + select |
| 线程模型 | async/await + mpsc channel | 替代 C++ 的 pthread + EMutex + 信号机制 |
| 回调模式 | Rust trait + enum Event | 替代 C++ 虚函数回调 (EWrapper) |
| 编码方式 | 手动二进制编码 | 与 C++ 保持协议兼容 (null-terminated ASCII) |
| Protobuf | prost crate | 对应 C++ 的 protobuf 支持 (MIN_SERVER_VER >= 201) |
| Decimal | rust_decimal crate | 替代 Intel BID64 库 |
| 错误处理 | thiserror + 自定义枚举 | Rust 惯用模式 |

---

## 已完成的任务（归档）

### [x] 阶段一：项目脚手架与基础类型 (完成日期: 2026-02-21)

- **完成摘要**: 创建 `vault-wolf-ibapi` crate 作为 workspace member，移植全部 C++ 数据结构（30+ struct）、协议常量（450+）、枚举类型（16 种）和错误类型。14 个单元测试全部通过，clippy 零 warning。
- **修改文件**:
  - `Cargo.toml` — 添加 `[workspace]` section
  - 新建 `vault-wolf-ibapi/` 目录（13 个文件，~2300 行 Rust 代码）:
    - `Cargo.toml`, `src/lib.rs`, `src/errors.rs`, `src/protocol.rs`
    - `src/models/mod.rs`, `enums.rs`, `common.rs`, `market_data.rs`, `bar.rs`, `scanner.rs`, `contract.rs`, `execution.rs`, `order.rs`

---

## 未完成的任务

### [ ] 阶段二：传输层与协议编解码

- **需求描述**: 创建独立 crate 项目结构，定义所有核心数据类型（对应 C++ 头文件中的 struct）
- **涉及文件**:
  - 新建 `vault-wolf-ibapi/Cargo.toml` - crate 配置
  - 新建 `vault-wolf-ibapi/src/lib.rs` - 公共 API
  - 新建 `vault-wolf-ibapi/src/models/*.rs` - 全部数据结构
  - 新建 `vault-wolf-ibapi/src/protocol.rs` - 协议常量
  - 新建 `vault-wolf-ibapi/src/errors.rs` - 错误类型
  - 修改 `Cargo.toml` (workspace 级别) - 添加 workspace member
- **操作步骤**:
  1. [ ] 创建 crate 目录结构和 `Cargo.toml`（依赖: tokio, serde, rust_decimal, thiserror, prost, bytes, tracing）
  2. [ ] 定义 `protocol.rs` - 移植所有协议常量
     - 消息类型 ID（TICK_PRICE=1, ORDER_STATUS=3 等 66 个入站消息）
     - 请求类型 ID（REQ_MKT_DATA=1, PLACE_ORDER=3 等 40+ 个出站请求）
     - MIN_SERVER_VER_* 常量（160+ 个版本门控常量）
     - CLIENT_VERSION = 66
  3. [ ] 定义 `models/contract.rs` - Contract, ComboLeg, DeltaNeutralContract, ContractDetails, ContractDescription
     - 参照 `cppclient/client/Contract.h`
     - 使用 Rust enum 替代字符串字段（SecType, Right 等）
  4. [ ] 定义 `models/order.rs` - Order, OrderState, OrderCancel, OrderCondition 及其子类型
     - 参照 `cppclient/client/Order.h`, `OrderState.h`, `OrderCondition.h`
     - OrderCondition 使用 Rust enum 替代 C++ 的继承层级（PriceCondition, TimeCondition, MarginCondition 等）
  5. [ ] 定义 `models/execution.rs` - Execution, ExecutionFilter, CommissionReport
     - 参照 `cppclient/client/Execution.h`, `CommissionReport.h`
  6. [ ] 定义 `models/bar.rs` - Bar, HistoricalTick, HistoricalTickBidAsk, HistoricalTickLast, HistoricalSession
     - 参照 `cppclient/client/bar.h`, `HistoricalTick*.h`
  7. [ ] 定义 `models/market_data.rs` - TickAttrib, TickAttribBidAsk, TickAttribLast, DepthMktDataDescription
  8. [ ] 定义 `models/account.rs` - FamilyCode, AccountSummary 等
  9. [ ] 定义 `models/common.rs` - TagValue, SoftDollarTier, NewsProvider, SmartComponent, Decimal 别名等
  10. [ ] 定义 `errors.rs` - IBApiError 枚举（连接错误、编码错误、协议错误、服务端错误）
  11. [ ] 编写 `lib.rs` 导出所有公共类型，确保 `cargo build` 通过

### [ ] 阶段二：传输层与协议编解码

- **需求描述**: 实现 TCP 连接管理和 IB 二进制协议的编码/解码，这是整个客户端的通信基础
- **涉及文件**:
  - 新建 `vault-wolf-ibapi/src/transport.rs` — TCP 传输
  - 新建 `vault-wolf-ibapi/src/encoder.rs` — 消息编码
  - 新建 `vault-wolf-ibapi/src/decoder.rs` — 消息解码（部分）
- **操作步骤**:
  1. [ ] 实现 `transport.rs` - TCP 传输层
     - 使用 `tokio::net::TcpStream` 进行异步连接
     - 实现消息帧处理：4 字节大端长度前缀 + 消息体
     - 实现写缓冲（对应 C++ `ESocket::m_outBuffer`）
     - 实现读缓冲和消息分帧（从字节流中切割完整消息）
     - 连接重定向支持（对应 `m_allowRedirect`，最多 2 次）
  2. [ ] 实现 `encoder.rs` - 请求编码器
     - `encode_field<T>()` 泛型编码：值 → ASCII 字符串 + `\0` 终止符
     - `encode_field_max(value)` - 当值为 MAX 时编码为空
     - `encode_contract()` - Contract 结构体编码
     - `encode_tag_value_list()` - TagValue 列表编码
     - `encode_order()` - Order 结构体编码（最复杂，200+ 字段，需要版本门控）
     - 消息构建器：构建完整消息（长度前缀 + 消息体）
  3. [ ] 实现 `decoder.rs` 基础框架
     - 字段解码：从 `\0` 分隔的字节流中提取 String / i32 / f64 / Decimal
     - `decode_field<T>()` 泛型解码
     - 消息头解析（消息类型 ID + 版本号）
     - 预留消息分发逻辑（具体的消息解码在阶段四完成）
  4. [ ] 实现连接握手
     - 发送 `"API\0"` 前缀 + 支持的版本范围
     - 接收服务端版本号和连接时间
     - 版本协商逻辑
     - 发送 startApi 消息（clientId + optionalCapabilities）
  5. [ ] 编写编码/解码单元测试
     - 测试各类型字段的编解码往返一致性
     - 测试消息帧的分割与重组
     - 测试连接握手消息的格式

### [ ] 阶段三：异步消息读取与事件分发

- **需求描述**: 实现异步消息读取器和事件分发系统，替代 C++ 的 EReader 线程 + EWrapper 回调模式
- **涉及文件**:
  - 新建 `vault-wolf-ibapi/src/reader.rs` - 异步消息读取
  - 新建 `vault-wolf-ibapi/src/wrapper.rs` - Wrapper trait 定义
  - 新建 `vault-wolf-ibapi/src/client.rs` - IBClient 基础结构
- **操作步骤**:
  1. [ ] 定义 `wrapper.rs` - IBWrapper trait
     - 定义 125+ 回调方法，全部提供默认空实现（`fn tick_price(&mut self, ...) {}`）
     - 按功能分组：市场数据、订单管理、账户、历史数据、深度行情、扫描器等
     - 使用 Rust 类型替代 C++ 的原始类型（如 `TickType` enum 替代 int）
     - 关键回调：`error()`, `connection_closed()`, `next_valid_id()`, `managed_accounts()`
  2. [ ] 或者 - 定义事件枚举方案 `IBEvent`
     - 考虑提供 enum 事件 + mpsc channel 的替代模式（更 Rust 化）
     - `enum IBEvent { TickPrice {...}, OrderStatus {...}, Error {...}, ... }`
     - 用户可通过 `while let Some(event) = rx.recv().await` 消费事件
     - 两种模式可共存：trait 回调 + channel 事件
  3. [ ] 实现 `reader.rs` - 异步消息读取器
     - 使用 `tokio::io::AsyncReadExt` 从 TcpStream 持续读取
     - 在独立 tokio task 中运行（`tokio::spawn`，替代 C++ 的 pthread）
     - 读取完整消息后通过 mpsc channel 发送给主处理逻辑
     - 处理连接断开和错误恢复
     - 心跳/超时检测
  4. [ ] 实现 `client.rs` - IBClient 基础结构
     - 管理连接状态（connected, server_version, client_id）
     - `connect()` / `disconnect()` 方法
     - 持有 transport 写半部分 + reader task handle
     - 提供 `next_req_id()` 原子递增

### [ ] 阶段四：消息解码器完整实现

- **需求描述**: 实现所有 66 种入站消息的完整解码逻辑，将二进制消息转换为结构化事件
- **涉及文件**:
  - `vault-wolf-ibapi/src/decoder.rs` - 完善消息解码
- **操作步骤**:
  1. [ ] 实现核心消息解码（优先级最高，vault-wolf 当前使用的功能）
     - `TICK_PRICE` (1) - 实时价格
     - `TICK_SIZE` (2) - 实时数量
     - `ORDER_STATUS` (3) - 订单状态变更
     - `ERR_MSG` (4) - 错误消息
     - `OPEN_ORDER` (5) - 开放订单详情
     - `ACCT_VALUE` (6) - 账户值更新
     - `PORTFOLIO_VALUE` (7) - 投资组合更新
     - `ACCT_UPDATE_TIME` (8) - 账户更新时间
     - `NEXT_VALID_ID` (9) - 下一个可用订单 ID
     - `EXECUTION_DATA` (11) - 成交回报
     - `HISTORICAL_DATA` (17) - 历史 K 线数据
     - `MANAGED_ACCTS` (15) - 管理账户列表
     - `POSITION_DATA` (61) - 持仓数据
     - `ACCOUNT_SUMMARY` (63) - 账户摘要
  2. [ ] 实现市场数据相关消息解码
     - `TICK_OPTION_COMPUTATION` (21) - 期权希腊值
     - `TICK_GENERIC` (45) - 通用 tick
     - `TICK_STRING` (46) - 字符串 tick
     - `TICK_EFP` (47) - EFP tick
     - `MARKET_DEPTH` (12) / `MARKET_DEPTH_L2` (13) - 深度行情
     - `TICK_BY_TICK` (99) - 逐笔行情
     - `TICK_REQ_PARAMS` (81) - tick 参数
  3. [ ] 实现订单与合约相关消息解码
     - `CONTRACT_DATA` (10) - 合约详情
     - `BOND_CONTRACT_DATA` (18) - 债券合约
     - `COMMISSION_REPORT` (59) - 佣金报告
     - `COMPLETED_ORDER` (101) - 已完成订单
     - `ORDER_BOUND` (97) - 订单绑定
  4. [ ] 实现其余消息解码（扫描器、新闻、高级功能）
     - `SCANNER_DATA` (20), `SCANNER_PARAMETERS` (19)
     - `NEWS_ARTICLE` (82), `NEWS_BULLETINS` (14)
     - `HISTORICAL_NEWS` / `HISTORICAL_NEWS_END`
     - `REAL_TIME_BARS` (50)
     - `FUNDAMENTAL_DATA` (51)
     - 其余所有消息类型
  5. [ ] Protobuf 消息解码支持
     - 使用 prost 编译 `.proto` 文件（从 `cppclient/protos/`）
     - 实现 protobuf 格式的订单、成交等消息解码
     - 版本门控：`server_version >= MIN_SERVER_VER_PROTOBUF (201)`

### [ ] 阶段五：请求方法完整实现

- **需求描述**: 在 IBClient 上实现所有 40+ 个请求方法，对应 C++ EClient 的公共接口
- **涉及文件**:
  - `vault-wolf-ibapi/src/client.rs` - 完善请求方法
  - `vault-wolf-ibapi/src/encoder.rs` - 可能需要补充编码逻辑
- **操作步骤**:
  1. [ ] 实现核心请求方法（vault-wolf 当前使用的）
     - `req_mkt_data()` / `cancel_mkt_data()` - 实时行情订阅
     - `place_order()` / `cancel_order()` - 下单/撤单
     - `req_open_orders()` / `req_all_open_orders()` - 查询订单
     - `req_account_updates()` - 账户更新订阅
     - `req_positions()` / `cancel_positions()` - 持仓查询
     - `req_account_summary()` / `cancel_account_summary()` - 账户摘要
     - `req_historical_data()` / `cancel_historical_data()` - 历史数据
     - `req_contract_details()` - 合约详情
     - `req_ids()` - 请求下一个有效 ID
     - `req_current_time()` - 服务器时间
  2. [ ] 实现市场数据请求
     - `req_mkt_depth()` / `cancel_mkt_depth()` - 深度行情
     - `req_real_time_bars()` / `cancel_real_time_bars()` - 实时 K 线
     - `req_tick_by_tick_data()` / `cancel_tick_by_tick_data()` - 逐笔
     - `req_historical_ticks()` - 历史逐笔
     - `req_scanner_subscription()` / `cancel_scanner_subscription()` - 扫描器
     - `req_scanner_parameters()` - 扫描器参数
  3. [ ] 实现高级请求
     - `req_executions()` - 成交查询
     - `req_fundamental_data()` - 基本面数据
     - `req_news_bulletins()` / `cancel_news_bulletins()` - 新闻
     - `req_managed_accts()` - 管理账户
     - `req_global_cancel()` - 全局撤单
     - `req_market_rule()` - 市场规则
     - 其余所有请求方法
  4. [ ] 实现 Protobuf 编码请求
     - PlaceOrder protobuf 编码 (`server_version >= 203`)
     - CancelOrder protobuf 编码
     - 其他 protobuf 请求

### [ ] 阶段六：集成测试与 vault-wolf 迁移

- **需求描述**: 编写集成测试确保与 TWS/Gateway 的协议兼容性，然后将 vault-wolf 从 `ibapi` crate 迁移到自建库
- **涉及文件**:
  - `vault-wolf-ibapi/tests/*.rs` - 集成测试
  - `vault-wolf/Cargo.toml` - 切换依赖
  - `vault-wolf/src/manager.rs` - 适配新 API
  - `vault-wolf/src/models.rs` - 可能需要调整
  - `vault-wolf/src/web.rs` - 可能需要调整
- **操作步骤**:
  1. [ ] 编写连接测试 - 验证能成功连接到 TWS Paper Trading
  2. [ ] 编写行情测试 - 订阅 SPY 实时行情，验证回调触发
  3. [ ] 编写下单测试 - 在 Paper 账户提交限价单，验证订单状态回调
  4. [ ] 编写历史数据测试 - 请求 SPY 日线，验证数据完整性
  5. [ ] 编写账户测试 - 请求账户摘要和持仓
  6. [ ] 修改 `vault-wolf/Cargo.toml`，将 `ibapi` 替换为 `vault-wolf-ibapi`（路径依赖）
  7. [ ] 适配 `manager.rs` - 将所有 `ibapi::*` 调用替换为 `vault_wolf_ibapi::*`
  8. [ ] 端到端测试 - 启动 vault-wolf 服务器，通过 REST API 验证全部功能
  9. [ ] 性能对比 - 与原 `ibapi` crate 的延迟和吞吐量对比

---

## 已完成的任务（归档）

（暂无）
