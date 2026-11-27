// Beamlet 核心原语: Operation, Attempt, Report.
// 零 I/O, 不包含上层状态机、集合管理、角色或传输概念.
module primitives {
  // Operation 不可变数据投影: 固定身份与原始请求.
  type Operation[o, p] = {
    id: o,
    request: p,
  }

  // 针对一个 Operation 的单次执行授权.
  type Attempt[o, a] = {
    id: a,
    operation_id: o,
  }

  // 一次 Attempt 执行结束后的结果证据, 保留双引用关联.
  type Report[o, a, r, v] = {
    id: r,
    operation_id: o,
    attempt_id: a,
    result: v,
  }

  // 构造 Operation 不可变投影.
  pure def makeOperation(operationId: o, req: p): Operation[o, p] =
    { id: operationId, request: req }

  // 构造绑定到指定 Operation 的 Attempt.
  pure def makeAttempt(operationId: o, attemptId: a): Attempt[o, a] =
    { id: attemptId, operation_id: operationId }

  // 构造关联 Operation 与 Attempt 身份的 Report.
  pure def makeReport(
    reportId: r,
    operationId: o,
    attemptId: a,
    value: v
  ): Report[o, a, r, v] =
    {
      id: reportId,
      operation_id: operationId,
      attempt_id: attemptId,
      result: value,
    }
}
