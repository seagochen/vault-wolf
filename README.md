# VaultWolf Trading API Server

**VaultWolf** 是一个基于 Interactive Brokers (IB) TWS API 的 C++ 交易系统，提供 RESTful Web API 接口，支持股票和期权的实时数据查询、历史数据获取、账户管理和订单操作。

## 目录

- [特性](#特性)
- [系统要求](#系统要求)
- [安装](#安装)
- [配置](#配置)
- [运行](#运行)
- [API 文档](#api-文档)
- [示例](#示例)
- [架构](#架构)
- [许可证](#许可证)

## 特性

### 市场数据
- ✅ **实时行情**：获取股票和期权的实时报价、成交价、买卖价等
- ✅ **历史数据**：查询任意时间段的历史K线数据（分钟、小时、日线等）
- ✅ **期权数据**：支持期权链查询、隐含波动率、Greeks 计算

### 账户管理
- ✅ **账户摘要**：查询账户余额、净值、可用资金等
- ✅ **持仓查询**：实时获取所有持仓信息
- ✅ **盈亏统计**：实时和历史盈亏跟踪

### 订单管理
- ✅ **下单**：支持市价单、限价单、止损单
- ✅ **撤单**：取消未成交订单
- ✅ **改单**：修改订单价格和数量
- ✅ **订单查询**：查看所有订单、历史订单、订单状态

### 技术特性
- ✅ **RESTful API**：标准 HTTP 接口，易于集成
- ✅ **JSON 格式**：所有响应均为 JSON 格式
- ✅ **线程安全**：多线程设计，支持并发请求
- ✅ **自动重连**：自动处理连接断开和重连

## 系统要求

### 软件要求
- **操作系统**: Linux (Ubuntu 20.04+ 推荐)
- **编译器**: GCC 7.0+ 或 Clang 5.0+ (支持 C++17)
- **CMake**: 3.10+
- **TWS 或 IB Gateway**: Interactive Brokers 交易平台

### 依赖库
- **IB TWS API**: C++ 客户端库（已包含在项目中）
- **cpp-httplib**: HTTP 服务器库（已包含）
- **Intel RDFP Math Library**: 十进制浮点运算库（已包含）
- **Pthreads**: POSIX 线程库

## 安装

### 1. 克隆仓库

```bash
git clone https://github.com/yourusername/VaultWolf.git
cd VaultWolf
```

### 2. 解压依赖库

```bash
cd thirds
tar -xzf IntelRDFPMathLib20U2.tar.gz
cd ..
```

### 3. 编译项目

```bash
mkdir -p build
cd build
cmake ..
make -j4
```

编译成功后，可执行文件 `VaultWolfApp` 将生成在项目根目录下。

## 配置

### 配置 TWS/IB Gateway

1. **启动 TWS 或 IB Gateway**
2. **启用 API 连接**：
   - TWS: File → Global Configuration → API → Settings
   - 勾选 "Enable ActiveX and Socket Clients"
   - 设置 Socket Port（默认 7497 实盘，4002 模拟盘）
   - 取消勾选 "Read-Only API"（如果需要下单功能）

3. **添加可信 IP**：
   - 在 "Trusted IPs" 中添加 `127.0.0.1`（或允许所有 IP）

## 运行

### 基本用法

```bash
# 使用默认参数运行（连接到 localhost:4002）
./VaultWolfApp

# 指定 TWS/Gateway 地址和端口
./VaultWolfApp --ib-host 127.0.0.1 --ib-port 4002

# 指定 Web 服务器端口
./VaultWolfApp --web-port 8080

# 完整参数示例
./VaultWolfApp --ib-host 192.168.1.100 --ib-port 7497 --ib-client-id 1 --web-port 5000
```

### 命令行参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--ib-host` | IB TWS/Gateway 主机地址 | 127.0.0.1 |
| `--ib-port` | IB TWS/Gateway 端口 | 4002 |
| `--ib-client-id` | IB 客户端 ID | 0 |
| `--web-port` | Web API 服务器端口 | 5000 |
| `--help` | 显示帮助信息 | - |

### 验证运行

服务器启动后，访问健康检查端点：

```bash
curl http://localhost:5000/health
```

预期响应：

```json
{
  "status": "healthy",
  "ibConnected": true,
  "server": "VaultWolf API Server",
  "version": "1.0.0"
}
```

## API 文档

### 基础信息

- **Base URL**: `http://localhost:5000`
- **Content-Type**: `application/json`
- **响应格式**: JSON

### 通用响应格式

#### 成功响应

```json
{
  "success": true,
  "message": "操作成功",
  "data": { ... }
}
```

#### 错误响应

```json
{
  "success": false,
  "message": "错误描述",
  "errorCode": 400
}
```

---

### 市场数据 API

#### 1. 获取实时行情

**GET** `/api/market/realtime`

**查询参数**:
- `symbol` (必需): 股票代码，如 "SPY"
- `sec_type` (可选): 证券类型，默认 "STK"（STK=股票，OPT=期权）
- `currency` (可选): 货币，默认 "USD"
- `exchange` (可选): 交易所，默认 "SMART"

对于期权，还需要：
- `right`: "C" (看涨) 或 "P" (看跌)
- `strike`: 行权价
- `expiry`: 到期日 (YYYYMMDD)

**示例**:

```bash
# 获取 SPY 股票实时行情
curl "http://localhost:5000/api/market/realtime?symbol=SPY"

# 获取期权实时行情
curl "http://localhost:5000/api/market/realtime?symbol=SPY&sec_type=OPT&right=C&strike=450&expiry=20250117"
```

**响应**:

```json
{
  "success": true,
  "message": "Market data retrieved",
  "data": {
    "symbol": "SPY",
    "secType": "STK",
    "reqId": 1000,
    "bid": 449.50,
    "ask": 449.51,
    "last": 449.50,
    "close": 448.90,
    "open": 449.00,
    "high": 450.20,
    "low": 448.75,
    "bidSize": 100,
    "askSize": 200,
    "lastSize": 50,
    "volume": 5432100,
    "timestamp": "2025-11-21 14:30:00"
  }
}
```

#### 2. 订阅实时行情

**POST** `/api/market/subscribe`

**查询参数**: 同上

**响应**:

```json
{
  "success": true,
  "message": "Market data subscription created",
  "data": {
    "reqId": 1001,
    "symbol": "AAPL",
    "secType": "STK"
  }
}
```

#### 3. 取消行情订阅

**POST** `/api/market/unsubscribe`

**查询参数**:
- `req_id` (必需): 订阅请求 ID

**示例**:

```bash
curl -X POST "http://localhost:5000/api/market/unsubscribe?req_id=1001"
```

#### 4. 获取历史数据

**GET** `/api/market/historical`

**查询参数**:
- `symbol` (必需): 股票代码
- `sec_type` (可选): 证券类型，默认 "STK"
- `end_date` (可选): 结束日期时间 (YYYYMMDD HH:MM:SS)，默认当前时间
- `duration` (可选): 持续时间，如 "1 D", "1 W", "1 M", "1 Y"，默认 "1 D"
- `bar_size` (可选): K线周期，如 "1 min", "5 mins", "1 hour", "1 day"，默认 "1 hour"
- `what_to_show` (可选): 数据类型，默认 "TRADES"
  - TRADES, MIDPOINT, BID, ASK, BID_ASK, etc.

**示例**:

```bash
# 获取 SPY 最近 1 天的小时K线
curl "http://localhost:5000/api/market/historical?symbol=SPY&duration=1%20D&bar_size=1%20hour"

# 获取 AAPL 最近 5 天的日K线
curl "http://localhost:5000/api/market/historical?symbol=AAPL&duration=5%20D&bar_size=1%20day"
```

**响应**:

```json
{
  "success": true,
  "message": "Historical data retrieved",
  "data": {
    "symbol": "SPY",
    "secType": "STK",
    "reqId": 2000,
    "startDate": "20251120",
    "endDate": "20251121",
    "bars": [
      {
        "date": "20251121  09:30:00",
        "open": 449.00,
        "high": 449.50,
        "low": 448.75,
        "close": 449.25,
        "volume": 1234567,
        "barCount": 8923,
        "wap": 449.12
      },
      ...
    ]
  }
}
```

---

### 账户 API

#### 5. 获取账户摘要

**GET** `/api/account/summary`

**查询参数**:
- `account` (可选): 账户 ID，默认返回第一个账户

**示例**:

```bash
curl "http://localhost:5000/api/account/summary"
```

**响应**:

```json
{
  "success": true,
  "message": "Account summary retrieved",
  "data": {
    "account": "DU1234567",
    "values": {
      "NetLiquidation": "100000.00",
      "TotalCashValue": "50000.00",
      "AvailableFunds": "75000.00",
      "BuyingPower": "300000.00",
      "GrossPositionValue": "50000.00",
      "RealizedPnL": "5000.00",
      "UnrealizedPnL": "2500.00"
    }
  }
}
```

#### 6. 获取持仓信息

**GET** `/api/account/positions`

**查询参数**:
- `account` (可选): 按账户筛选
- `symbol` (可选): 按股票代码筛选
- `sec_type` (可选): 按证券类型筛选

**示例**:

```bash
# 获取所有持仓
curl "http://localhost:5000/api/account/positions"

# 获取 SPY 持仓
curl "http://localhost:5000/api/account/positions?symbol=SPY"
```

**响应**:

```json
{
  "success": true,
  "message": "Positions retrieved",
  "data": [
    {
      "account": "DU1234567",
      "symbol": "SPY",
      "secType": "STK",
      "currency": "USD",
      "exchange": "ARCA",
      "position": 100.0,
      "avgCost": 445.50,
      "marketPrice": 449.50,
      "marketValue": 44950.0,
      "unrealizedPNL": 400.0,
      "realizedPNL": 0.0
    },
    ...
  ]
}
```

---

### 订单 API

#### 7. 下单

**POST** `/api/order/place`

**查询参数**:
- `symbol` (必需): 股票代码
- `action` (必需): "BUY" 或 "SELL"
- `quantity` (必需): 数量
- `order_type` (可选): 订单类型，默认 "MKT"
  - MKT = 市价单
  - LMT = 限价单
  - STP = 止损单
- `limit_price` (限价单必需): 限价价格
- `stop_price` (止损单必需): 止损价格
- `sec_type`, `currency`, `exchange` (可选): 同市场数据 API

对于期权：
- `right`: "C" 或 "P"
- `strike`: 行权价
- `expiry`: 到期日

**示例**:

```bash
# 市价买入 100 股 SPY
curl -X POST "http://localhost:5000/api/order/place?symbol=SPY&action=BUY&quantity=100&order_type=MKT"

# 限价卖出 50 股 AAPL，价格 180.00
curl -X POST "http://localhost:5000/api/order/place?symbol=AAPL&action=SELL&quantity=50&order_type=LMT&limit_price=180.00"

# 止损卖出 100 股 TSLA，止损价 250.00
curl -X POST "http://localhost:5000/api/order/place?symbol=TSLA&action=SELL&quantity=100&order_type=STP&stop_price=250.00"
```

**响应**:

```json
{
  "success": true,
  "message": "Order placed successfully",
  "data": {
    "orderId": 1,
    "symbol": "SPY",
    "action": "BUY",
    "quantity": 100.0,
    "orderType": "MKT"
  }
}
```

#### 8. 撤单

**POST** `/api/order/cancel`

**查询参数**:
- `order_id` (必需): 订单 ID

**示例**:

```bash
curl -X POST "http://localhost:5000/api/order/cancel?order_id=1"
```

**响应**:

```json
{
  "success": true,
  "message": "Order cancellation requested"
}
```

#### 9. 改单

**POST** `/api/order/modify`

**查询参数**:
- `order_id` (必需): 订单 ID
- `quantity` (可选): 新数量
- `limit_price` (可选): 新限价价格
- `stop_price` (可选): 新止损价格

**示例**:

```bash
curl -X POST "http://localhost:5000/api/order/modify?order_id=1&quantity=150&limit_price=180.50"
```

**响应**:

```json
{
  "success": true,
  "message": "Order modification requested"
}
```

#### 10. 查询订单列表

**GET** `/api/order/list`

**查询参数**:
- `status` (可选): 按状态筛选，如 "Filled", "Submitted", "Cancelled"
- `symbol` (可选): 按股票代码筛选
- `sec_type` (可选): 按证券类型筛选

**示例**:

```bash
# 获取所有订单
curl "http://localhost:5000/api/order/list"

# 获取已成交订单
curl "http://localhost:5000/api/order/list?status=Filled"

# 获取 SPY 的订单
curl "http://localhost:5000/api/order/list?symbol=SPY"
```

**响应**:

```json
{
  "success": true,
  "message": "Orders retrieved",
  "data": [
    {
      "orderId": 1,
      "account": "DU1234567",
      "symbol": "SPY",
      "secType": "STK",
      "exchange": "SMART",
      "currency": "USD",
      "action": "BUY",
      "orderType": "MKT",
      "totalQuantity": 100.0,
      "lmtPrice": 0.0,
      "auxPrice": 0.0,
      "status": "Filled",
      "filled": 100.0,
      "remaining": 0.0,
      "avgFillPrice": 449.52,
      "permId": 12345,
      "parentId": 0,
      "lastFillPrice": 449.52,
      "submitTime": "2025-11-21 14:30:00",
      "lastUpdateTime": "2025-11-21 14:30:05"
    },
    ...
  ]
}
```

#### 11. 查询单个订单

**GET** `/api/order/:id`

**路径参数**:
- `id`: 订单 ID

**示例**:

```bash
curl "http://localhost:5000/api/order/1"
```

**响应**: 同上单个订单对象

---

## 示例

### Python 示例

```python
import requests

BASE_URL = "http://localhost:5000"

# 1. 获取实时行情
response = requests.get(f"{BASE_URL}/api/market/realtime", params={
    "symbol": "SPY"
})
print(response.json())

# 2. 获取历史数据
response = requests.get(f"{BASE_URL}/api/market/historical", params={
    "symbol": "AAPL",
    "duration": "5 D",
    "bar_size": "1 day"
})
print(response.json())

# 3. 获取账户摘要
response = requests.get(f"{BASE_URL}/api/account/summary")
print(response.json())

# 4. 获取持仓
response = requests.get(f"{BASE_URL}/api/account/positions")
print(response.json())

# 5. 下市价单
response = requests.post(f"{BASE_URL}/api/order/place", params={
    "symbol": "SPY",
    "action": "BUY",
    "quantity": 10,
    "order_type": "MKT"
})
print(response.json())

# 6. 下限价单
response = requests.post(f"{BASE_URL}/api/order/place", params={
    "symbol": "AAPL",
    "action": "SELL",
    "quantity": 5,
    "order_type": "LMT",
    "limit_price": 180.00
})
order_id = response.json()["data"]["orderId"]

# 7. 查询订单
response = requests.get(f"{BASE_URL}/api/order/list")
print(response.json())

# 8. 撤单
response = requests.post(f"{BASE_URL}/api/order/cancel", params={
    "order_id": order_id
})
print(response.json())
```

### curl 示例

详见上述 API 文档中的示例。

---

## 架构

### 项目结构

```
VaultWolf/
├── include/                    # 头文件目录
│   ├── common/                 # 公共数据类型和工具
│   │   ├── data_types.h        # 数据结构定义
│   │   └── json_helper.h       # JSON 序列化工具
│   ├── ibwrapper/              # IB API 封装
│   │   └── vault_ewrapper.h    # IB EWrapper 封装类
│   ├── manager/                # 业务管理层
│   │   └── vault_manager.h     # VaultWolfManager 类
│   └── web/                    # Web 服务层
│       └── web_server.h        # WebServer 类
├── src/                        # 源代码目录
│   ├── ibwrapper/              # IB API 封装实现
│   ├── manager/                # 业务管理层实现
│   ├── web/                    # Web 服务层实现
│   └── main_server.cpp         # 主程序入口
├── IBTwsApi/                   # IB TWS API 库
├── thirds/                     # 第三方库
└── CMakeLists.txt              # CMake 配置文件
```

### 系统架构

```
用户请求 (HTTP)
       ↓
HTTP Server (cpp-httplib)
       ↓
API Router & Handler
       ↓
Manager Layer:
  - DataManager (实时/历史数据)
  - OrderManager (订单管理)
  - AccountManager (账户查询)
       ↓
VaultEWrapper (IB API 封装)
       ↓
IB TWS/Gateway API
```

### 核心模块

1. **VaultWolfManager** (`manager/vault_manager.h`)
   - 继承自 `VaultEWrapper`
   - 提供数据存储和查询接口
   - 线程安全的数据管理

2. **WebServer** (`web/web_server.h`)
   - 基于 cpp-httplib 的 HTTP 服务器
   - RESTful API 路由
   - JSON 请求/响应处理

3. **Data Types** (`common/data_types.h`)
   - 标准化的数据结构
   - TickData, HistoricalData, OrderInfo, Position 等

4. **JSON Helper** (`common/json_helper.h`)
   - 轻量级 JSON 序列化工具
   - 无外部依赖

---

## 故障排除

### 常见问题

**问题 1**: 无法连接到 TWS/Gateway

```
Failed to connect to IB TWS/Gateway!
```

**解决方案**:
- 确保 TWS 或 IB Gateway 正在运行
- 检查 TWS API 设置是否启用
- 确认端口号正确（实盘 7497，模拟盘 4002）
- 检查防火墙设置

**问题 2**: 编译错误

```
fatal error: EClientSocket.h: No such file or directory
```

**解决方案**:
- 确保 `IBTwsApi` 目录存在
- 检查 CMake 配置是否正确
- 重新运行 `cmake ..`

**问题 3**: 运行时崩溃

**解决方案**:
- 检查 IB API 版本兼容性
- 确保 libbid.so 已正确编译
- 查看日志输出定位问题

---

## 许可证

本项目采用 GNU General Public License v3.0 (GPLv3) 许可证。详见 [LICENSE](LICENSE) 文件。

这意味着：
- ✅ 您可以自由使用、修改和分发本软件
- ✅ 您可以将其用于商业目的
- ⚠️ 如果您分发修改版本，必须同样以 GPLv3 许可证开源
- ⚠️ 您必须保留原始版权声明和许可证声明
- ⚠️ 任何基于本软件的衍生作品也必须采用 GPLv3 许可证

---

## 免责声明

⚠️ **重要提示**：

1. 本软件仅供学习和研究使用
2. 使用本软件进行实盘交易需自担风险
3. 作者不对使用本软件造成的任何损失负责
4. 请在模拟账户中充分测试后再考虑实盘使用
5. 请遵守相关法律法规和交易所规则

---

## 联系方式

- **作者**: VaultWolf Team
- **GitHub**: https://github.com/yourusername/VaultWolf
- **Email**: your.email@example.com

---

## 致谢

- [Interactive Brokers](https://www.interactivebrokers.com/) - 提供 TWS API
- [cpp-httplib](https://github.com/yhirose/cpp-httplib) - HTTP 服务器库
- [Intel RDFP Math Library](https://www.intel.com/) - 十进制浮点运算库

---

**Happy Trading! 🚀**
