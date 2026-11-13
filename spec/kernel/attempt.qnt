module attempt {
  /// 一次执行授权, o 和 a 分别为 Operation 与 Attempt 的身份类型.
  type Attempt[o, a] = { id: a, operation_id: o }

  /// 构造关联指定 Operation 的 Attempt 数据.
  pure def makeAttempt(operationId: o, id: a): Attempt[o, a] =
    { id: id, operation_id: operationId }
}
