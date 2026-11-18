# Beamlet

定义 Beamlet 的执行和节点装配术语.

## Language

**Ledger（执行台账）**:
一组 Operation 的身份、原始请求、当前状态及已签发 Attempt 身份的记录. 台账不表示持久化保证或完整事件历史.
_Avoid_: Runtime、Operations（用于指代台账时）

**Infrastructure (基础设施)**
由 Node 托管的支撑服务. Node 负责它的启动、运行和停止, 例如内嵌的消息路由服务.

**Transport (消息传输)**
应用为角色或客户端选择并绑定的消息收发方式. 同一传输可供多个角色共用, 也可连接其他 Node 托管的消息服务.
