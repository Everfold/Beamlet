module report {
  /// 执行证据, r 为报告身份类型, v 为结果内容类型.
  type Report[o, a, r, v] = {
    id: r,
    operation_id: o,
    attempt_id: a,
    result: v
  }

  /// 构造最终报告数据, 是否接受由状态机判断.
  pure def makeReport(id: r, operationId: o, attemptId: a, result: v): Report[o, a, r, v] =
    { id: id, operation_id: operationId, attempt_id: attemptId, result: result }
}
